use std::net::SocketAddr;
use std::sync::Arc;

use collab::core::collab::CollabOrigin;
use collab::core::collab_awareness::MutexCollabAwareness;
use collab::plugin_impl::rocks_disk::RocksDiskPlugin;
use collab::preclude::MapRefExtension;
use collab_persistence::kv::rocks_kv::RocksCollabDB;
use collab_plugins::sync_plugin::SyncPlugin;
use collab_sync::msg_codec::{CollabMsgCodec, CollabSink, CollabStream};
use tempfile::TempDir;
use tokio::net::TcpSocket;

pub async fn spawn_client(
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

pub async fn spawn_client_with_disk(
  origin: CollabOrigin,
  object_id: &str,
  address: SocketAddr,
  db: Option<Arc<RocksCollabDB>>,
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
  let db = db.unwrap_or_else(|| {
    let tempdir = TempDir::new().unwrap();
    let path = tempdir.into_path();
    Arc::new(RocksCollabDB::open(path).unwrap())
  });
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
