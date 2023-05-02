use std::net::SocketAddr;
use std::ops::Deref;
use std::sync::Arc;

use crate::util::{TestSink, TestStream};
use collab::core::collab::CollabOrigin;
use collab::core::collab_awareness::MutexCollabAwareness;
use collab::plugin_impl::rocks_disk::RocksDiskPlugin;
use collab::preclude::MapRefExtension;
use collab_persistence::kv::rocks_kv::RocksCollabDB;
use collab_plugins::sync_plugin::SyncPlugin;
use collab_sync::client::{TokioUnboundedSink, TokioUnboundedStream};
use collab_sync::msg_codec::{CollabMsgCodec, CollabSink, CollabStream};
use tempfile::TempDir;
use tokio::net::TcpSocket;
use tokio::sync::mpsc::unbounded_channel;

pub async fn spawn_client_with_empty_doc(
  origin: CollabOrigin,
  object_id: &str,
  address: SocketAddr,
) -> std::io::Result<Arc<MutexCollabAwareness>> {
  let stream = TcpSocket::new_v4()?.connect(address).await?;
  let (reader, writer) = stream.into_split();
  let collab = Arc::new(MutexCollabAwareness::new(origin.clone(), object_id, vec![]));

  let stream = CollabStream::new(reader, CollabMsgCodec::default());
  let sink = CollabSink::new(writer, CollabMsgCodec::default());
  let sync_plugin = SyncPlugin::new(origin, object_id, collab.clone(), sink, stream);
  collab.lock().collab.add_plugin(Arc::new(sync_plugin));
  collab.initial();

  Ok(collab)
}

pub async fn spawn_client(
  origin: CollabOrigin,
  object_id: &str,
  address: SocketAddr,
) -> std::io::Result<(Arc<RocksCollabDB>, Arc<MutexCollabAwareness>)> {
  let stream = TcpSocket::new_v4()?.connect(address).await?;
  let (reader, writer) = stream.into_split();
  let collab = Arc::new(MutexCollabAwareness::new(origin.clone(), object_id, vec![]));

  // sync
  let stream = CollabStream::new(reader, CollabMsgCodec::default());
  let sink = CollabSink::new(writer, CollabMsgCodec::default());
  let sync_plugin = SyncPlugin::new(origin.clone(), object_id, collab.clone(), sink, stream);
  collab.lock().collab.add_plugin(Arc::new(sync_plugin));

  // disk
  let tempdir = TempDir::new().unwrap();
  let path = tempdir.into_path();
  let db = Arc::new(RocksCollabDB::open(path).unwrap());
  let disk_plugin = RocksDiskPlugin::new(origin.uid, db.clone()).unwrap();
  collab.lock().collab.add_plugin(Arc::new(disk_plugin));
  collab.initial();

  {
    let client = collab.lock();
    client.collab.with_transact_mut(|txn| {
      let map = client.collab.create_map_with_txn(txn, "map");
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
  pub collab: Arc<MutexCollabAwareness>,
}

impl TestClient {
  pub async fn new(
    origin: CollabOrigin,
    object_id: &str,
    address: SocketAddr,
  ) -> std::io::Result<Self> {
    let stream = TcpSocket::new_v4()?.connect(address).await?;
    let (reader, writer) = stream.into_split();
    let collab = Arc::new(MutexCollabAwareness::new(origin.clone(), object_id, vec![]));

    // stream
    let tcp_stream = CollabStream::new(reader, CollabMsgCodec::default());
    let (tx, stream) = unbounded_channel();
    let test_stream = TestStream::new(tcp_stream, tx);

    // sink
    let tck_sink = CollabSink::new(writer, CollabMsgCodec::default());
    let (sink, rx) = unbounded_channel();
    let test_sink = TestSink::new(tck_sink, rx);

    let sync_plugin = SyncPlugin::new(
      origin.clone(),
      object_id,
      collab.clone(),
      TokioUnboundedSink(sink),
      TokioUnboundedStream::new(stream),
    );
    collab.lock().collab.add_plugin(Arc::new(sync_plugin));

    // disk
    let tempdir = TempDir::new().unwrap();
    let path = tempdir.into_path();
    let db = Arc::new(RocksCollabDB::open(path).unwrap());
    let disk_plugin = RocksDiskPlugin::new(origin.uid, db).unwrap();
    collab.lock().collab.add_plugin(Arc::new(disk_plugin));
    collab.initial();

    {
      let client = collab.lock();
      client.collab.with_transact_mut(|txn| {
        let map = client.collab.create_map_with_txn(txn, "map");
        map.insert_with_txn(txn, "task1", "a");
        map.insert_with_txn(txn, "task2", "b");
      });
    }
    Ok(Self {
      test_stream,
      test_sink,
      collab,
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
  type Target = Arc<MutexCollabAwareness>;

  fn deref(&self) -> &Self::Target {
    &self.collab
  }
}
