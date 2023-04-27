use std::net::SocketAddr;
use std::sync::Arc;
use tempfile::TempDir;

use collab::core::collab_awareness::MutexCollabAwareness;
use collab::plugin_impl::rocks_disk::RocksDiskPlugin;
use collab::preclude::MapRefExtension;
use collab_persistence::kv::rocks_kv::RocksCollabDB;
use collab_plugins::sync_plugin::SyncPlugin;
use tokio::net::TcpSocket;

use crate::util::{CollabMsgCodec, WrappedSink, WrappedStream};

pub async fn spawn_client(
  uid: i64,
  object_id: &str,
  address: SocketAddr,
) -> std::io::Result<Arc<MutexCollabAwareness>> {
  let stream = TcpSocket::new_v4()?.connect(address).await?;
  let (reader, writer) = stream.into_split();
  let collab = Arc::new(MutexCollabAwareness::new(uid, object_id, vec![]));

  let stream = WrappedStream::new(reader, CollabMsgCodec::default());
  let sink = WrappedSink::new(writer, CollabMsgCodec::default());
  let sync_plugin = SyncPlugin::new(uid, object_id, collab.clone(), sink, stream);
  collab.lock().collab.add_plugin(Arc::new(sync_plugin));
  collab.initial();

  Ok(collab)
}

pub async fn spawn_client_with_disk(
  uid: i64,
  object_id: &str,
  address: SocketAddr,
  db: Option<Arc<RocksCollabDB>>,
) -> std::io::Result<(Arc<RocksCollabDB>, Arc<MutexCollabAwareness>)> {
  let stream = TcpSocket::new_v4()?.connect(address).await?;
  let (reader, writer) = stream.into_split();
  let collab = Arc::new(MutexCollabAwareness::new(uid, object_id, vec![]));

  // sync
  let stream = WrappedStream::new(reader, CollabMsgCodec::default());
  let sink = WrappedSink::new(writer, CollabMsgCodec::default());
  let sync_plugin = SyncPlugin::new(uid, object_id, collab.clone(), sink, stream);
  collab.lock().collab.add_plugin(Arc::new(sync_plugin));

  // disk
  let db = db.unwrap_or_else(|| {
    let tempdir = TempDir::new().unwrap();
    let path = tempdir.into_path();
    Arc::new(RocksCollabDB::open(path).unwrap())
  });
  let disk_plugin = RocksDiskPlugin::new(uid, db.clone()).unwrap();
  collab.lock().collab.add_plugin(Arc::new(disk_plugin));

  collab.initial();
  Ok((db, collab))
}

pub async fn create_local_disk_document(
  uid: i64,
  object_id: &str,
  address: SocketAddr,
) -> Arc<RocksCollabDB> {
  let (db, client) = spawn_client_with_disk(uid, object_id, address, None)
    .await
    .unwrap();

  {
    let client = client.lock();
    client.collab.with_transact_mut(|txn| {
      let map = client.collab.create_map_with_txn(txn, "map");
      map.insert_with_txn(txn, "task1", "a");
      map.insert_with_txn(txn, "task2", "b");
    });
  }

  db
}
