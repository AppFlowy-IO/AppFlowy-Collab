use std::net::SocketAddr;
use std::ops::Deref;
use std::sync::Arc;

use collab::core::collab::MutexCollab;
use collab::core::origin::{CollabClient, CollabOrigin};
use collab::preclude::MapRefExtension;
use collab_persistence::kv::rocks_kv::RocksCollabDB;
use collab_plugins::sync::SyncPlugin;
use collab_sync::client::{TokioUnboundedSink, TokioUnboundedStream};
use collab_sync::server::{CollabMsgCodec, CollabSink, CollabStream};
use rand::{prelude::*, Rng as WrappedRng};
use tokio::net::{TcpSocket, TcpStream};
use tokio::sync::mpsc::unbounded_channel;

use collab_plugins::local_storage::rocksdb::RocksdbDiskPlugin;
use tempfile::TempDir;

use crate::util::{TestSink, TestStream};

pub async fn spawn_client_with_empty_doc(
  object_id: &str,
  address: SocketAddr,
) -> std::io::Result<Arc<MutexCollab>> {
  let stream = TcpSocket::new_v4()?.connect(address).await?;
  let origin = origin_from_tcp_stream(&stream);
  let (reader, writer) = stream.into_split();

  let collab = Arc::new(MutexCollab::new(origin.clone(), object_id, vec![]));
  let stream = CollabStream::new(reader, CollabMsgCodec::default());
  let sink = CollabSink::new(writer, CollabMsgCodec::default());
  let sync_plugin = SyncPlugin::new(origin, object_id, Arc::downgrade(&collab), sink, stream);
  collab.lock().add_plugin(Arc::new(sync_plugin));
  collab.lock().initialize();
  Ok(collab)
}

pub async fn spawn_client(
  uid: i64,
  object_id: &str,
  address: SocketAddr,
) -> std::io::Result<(Arc<RocksCollabDB>, Arc<MutexCollab>)> {
  let stream = TcpSocket::new_v4()?.connect(address).await?;
  let origin = origin_from_tcp_stream(&stream);
  let (reader, writer) = stream.into_split();
  let collab = Arc::new(MutexCollab::new(origin.clone(), object_id, vec![]));

  // sync
  let stream = CollabStream::new(reader, CollabMsgCodec::default());
  let sink = CollabSink::new(writer, CollabMsgCodec::default());
  let sync_plugin = SyncPlugin::new(origin, object_id, Arc::downgrade(&collab), sink, stream);
  collab.lock().add_plugin(Arc::new(sync_plugin));

  // disk
  let tempdir = TempDir::new().unwrap();
  let path = tempdir.into_path();
  let db = Arc::new(RocksCollabDB::open(path).unwrap());
  let disk_plugin = RocksdbDiskPlugin::new(uid, Arc::downgrade(&db));
  collab.lock().add_plugin(Arc::new(disk_plugin));
  collab.lock().initialize();

  {
    let client = collab.lock();
    client.with_transact_mut(|txn| {
      let map = client.insert_map_with_txn(txn, "map");
      map.insert_with_txn(txn, "task1", "a");
      map.insert_with_txn(txn, "task2", "b");
    });
  }

  Ok((db, collab))
}

pub struct TestClient {
  #[allow(dead_code)]
  test_stream: TestStream,
  #[allow(dead_code)]
  test_sink: TestSink,
  pub db: Arc<RocksCollabDB>,
  pub collab: Arc<MutexCollab>,
}

impl TestClient {
  pub async fn new(
    origin: CollabClient,
    object_id: &str,
    address: SocketAddr,
    with_data: bool,
  ) -> std::io::Result<Self> {
    let db = create_db();
    let uid = origin.uid;
    let stream = TcpSocket::new_v4()?.connect(address).await?;
    let origin = origin_from_tcp_stream(&stream);
    let (reader, writer) = stream.into_split();

    // disk
    let disk_plugin = RocksdbDiskPlugin::new(uid, Arc::downgrade(&db));
    let collab = Arc::new(MutexCollab::new(origin.clone(), object_id, vec![]));
    collab.lock().add_plugin(Arc::new(disk_plugin));

    // stream
    let tcp_stream = CollabStream::new(reader, CollabMsgCodec::default());
    let (tx, stream) = unbounded_channel();
    let test_stream = TestStream::new(tcp_stream, tx);

    // sink
    let tcp_sink = CollabSink::new(writer, CollabMsgCodec::default());
    let (sink, rx) = unbounded_channel();
    let test_sink = TestSink::new(tcp_sink, rx);

    let sync_plugin = SyncPlugin::new(
      origin,
      object_id,
      Arc::downgrade(&collab),
      TokioUnboundedSink(sink),
      TokioUnboundedStream::new(stream),
    );
    collab.lock().add_plugin(Arc::new(sync_plugin));
    collab.lock().initialize();
    if with_data {
      {
        let client = collab.lock();
        client.with_transact_mut(|txn| {
          let map = client.insert_map_with_txn(txn, "map");
          map.insert_with_txn(txn, "task1", "a");
          map.insert_with_txn(txn, "task2", "b");
        });
      }
    }
    Ok(Self {
      test_stream,
      test_sink,
      collab,
      db,
    })
  }

  pub async fn with_db(
    origin: CollabClient,
    object_id: &str,
    address: SocketAddr,
    db: Arc<RocksCollabDB>,
  ) -> std::io::Result<Self> {
    let uid = origin.uid;
    let stream = TcpSocket::new_v4()?.connect(address).await?;
    let origin = origin_from_tcp_stream(&stream);
    let (reader, writer) = stream.into_split();
    // disk
    let disk_plugin = RocksdbDiskPlugin::new(uid, Arc::downgrade(&db));
    let collab = Arc::new(MutexCollab::new(origin.clone(), object_id, vec![]));
    collab.lock().add_plugin(Arc::new(disk_plugin));

    // stream
    let tcp_stream = CollabStream::new(reader, CollabMsgCodec::default());
    let (tx, stream) = unbounded_channel();
    let test_stream = TestStream::new(tcp_stream, tx);

    // sink
    let tck_sink = CollabSink::new(writer, CollabMsgCodec::default());
    let (sink, rx) = unbounded_channel();
    let test_sink = TestSink::new(tck_sink, rx);

    let sync_plugin = SyncPlugin::new(
      origin,
      object_id,
      Arc::downgrade(&collab),
      TokioUnboundedSink(sink),
      TokioUnboundedStream::new(stream),
    );
    collab.lock().add_plugin(Arc::new(sync_plugin));
    collab.lock().initialize();
    Ok(Self {
      test_stream,
      test_sink,
      collab,
      db,
    })
  }

  pub fn disconnect(&mut self) {
    self.test_stream.disconnect();
    self.test_sink.disconnect();
  }

  pub fn connect(&mut self) {
    self.test_stream.connect();
    self.test_sink.connect();
  }
}

impl Deref for TestClient {
  type Target = Arc<MutexCollab>;

  fn deref(&self) -> &Self::Target {
    &self.collab
  }
}

pub fn create_db() -> Arc<RocksCollabDB> {
  let tempdir = TempDir::new().unwrap();
  let path = tempdir.into_path();
  Arc::new(RocksCollabDB::open(path).unwrap())
}

pub struct Rng(StdRng);

impl Default for Rng {
  fn default() -> Self {
    Rng(StdRng::from_rng(thread_rng()).unwrap())
  }
}

impl Rng {
  #[allow(dead_code)]
  pub fn from_seed(seed: [u8; 32]) -> Self {
    Rng(StdRng::from_seed(seed))
  }

  pub fn gen_string(&mut self, len: usize) -> String {
    (0..len)
      .map(|_| {
        let c = self.0.gen::<char>();
        format!("{:x}", c as u32)
      })
      .collect()
  }
}

fn origin_from_tcp_stream(stream: &TcpStream) -> CollabOrigin {
  let address = stream.local_addr().unwrap();
  let origin = CollabClient::new(address.port() as i64, &address.to_string());
  CollabOrigin::Client(origin)
}

pub fn generate_random_string(length: usize) -> String {
  const CHARSET: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
  let mut rng = rand::thread_rng();
  let random_string: String = (0..length)
    .map(|_| {
      let index = rng.gen_range(0..CHARSET.len());
      CHARSET[index] as char
    })
    .collect();

  random_string
}
