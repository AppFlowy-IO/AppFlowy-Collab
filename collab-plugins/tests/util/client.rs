use std::net::SocketAddr;
use std::ops::Deref;
use std::sync::Arc;

use collab::core::collab::MutexCollab;
use collab::plugin_impl::rocks_disk::RocksDiskPlugin;
use collab::preclude::MapRefExtension;
use collab_persistence::kv::rocks_kv::RocksCollabDB;
use collab_plugins::sync_plugin::SyncPlugin;
use collab_sync::client::{TokioUnboundedSink, TokioUnboundedStream};
use collab_sync::server::{CollabMsgCodec, CollabSink, CollabStream};

use collab::core::origin::{CollabClient, CollabOrigin};
use rand::{prelude::*, Rng as WrappedRng};
use tempfile::TempDir;
use tokio::net::TcpSocket;
use tokio::sync::mpsc::unbounded_channel;

use crate::util::{TestSink, TestStream};

pub async fn spawn_client_with_empty_doc(
  origin: CollabClient,
  object_id: &str,
  address: SocketAddr,
) -> std::io::Result<Arc<MutexCollab>> {
  let stream = TcpSocket::new_v4()?.connect(address).await?;
  let (reader, writer) = stream.into_split();
  let origin = CollabOrigin::Client(origin);
  let collab = Arc::new(MutexCollab::new(origin.clone(), object_id, vec![]));

  let stream = CollabStream::new(reader, CollabMsgCodec::default());
  let sink = CollabSink::new(writer, CollabMsgCodec::default());
  let sync_plugin = SyncPlugin::new(origin, object_id, collab.clone(), sink, stream);
  collab.lock().add_plugin(Arc::new(sync_plugin));
  collab.initial();
  Ok(collab)
}

pub async fn spawn_client(
  origin: CollabClient,
  object_id: &str,
  address: SocketAddr,
) -> std::io::Result<(Arc<RocksCollabDB>, Arc<MutexCollab>)> {
  let uid = origin.uid;
  let origin = CollabOrigin::Client(origin);
  let stream = TcpSocket::new_v4()?.connect(address).await?;
  let (reader, writer) = stream.into_split();
  let collab = Arc::new(MutexCollab::new(origin.clone(), object_id, vec![]));

  // sync
  let stream = CollabStream::new(reader, CollabMsgCodec::default());
  let sink = CollabSink::new(writer, CollabMsgCodec::default());
  let sync_plugin = SyncPlugin::new(origin.clone(), object_id, collab.clone(), sink, stream);
  collab.lock().add_plugin(Arc::new(sync_plugin));

  // disk
  let tempdir = TempDir::new().unwrap();
  let path = tempdir.into_path();
  let db = Arc::new(RocksCollabDB::open(path).unwrap());
  let disk_plugin = RocksDiskPlugin::new(uid, db.clone()).unwrap();
  collab.lock().add_plugin(Arc::new(disk_plugin));
  collab.initial();

  {
    let client = collab.lock();
    client.with_transact_mut(|txn| {
      let map = client.create_map_with_txn(txn, "map");
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
    let stream = TcpSocket::new_v4()?.connect(address).await?;
    let (reader, writer) = stream.into_split();
    // disk
    let disk_plugin = RocksDiskPlugin::new(origin.uid, db.clone()).unwrap();
    let origin = CollabOrigin::Client(origin);
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
      collab.clone(),
      TokioUnboundedSink(sink),
      TokioUnboundedStream::new(stream),
    );
    collab.lock().add_plugin(Arc::new(sync_plugin));

    collab.initial();
    if with_data {
      {
        let client = collab.lock();
        client.with_transact_mut(|txn| {
          let map = client.create_map_with_txn(txn, "map");
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
    let stream = TcpSocket::new_v4()?.connect(address).await?;
    let (reader, writer) = stream.into_split();
    // disk
    let disk_plugin = RocksDiskPlugin::new(origin.uid, db.clone()).unwrap();
    let origin = CollabOrigin::Client(origin);
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
      collab.clone(),
      TokioUnboundedSink(sink),
      TokioUnboundedStream::new(stream),
    );
    collab.lock().add_plugin(Arc::new(sync_plugin));

    collab.initial();
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
