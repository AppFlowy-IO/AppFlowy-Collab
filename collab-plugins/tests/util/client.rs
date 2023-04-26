use std::net::SocketAddr;
use std::sync::Arc;

use collab::core::collab_awareness::MutexCollabAwareness;
use collab_plugins::sync_plugin::SyncPlugin;
use tokio::net::TcpSocket;

use crate::util::{WrappedSink, WrappedStream, YrsCodec};

pub async fn make_test_client(
  uid: i64,
  doc_id: &str,
  address: SocketAddr,
) -> std::io::Result<Arc<MutexCollabAwareness>> {
  let stream = TcpSocket::new_v4()?.connect(address).await?;
  let (reader, writer) = stream.into_split();
  let collab = Arc::new(MutexCollabAwareness::new(uid, doc_id, vec![]));

  let stream = WrappedStream::new(reader, YrsCodec::default());
  let sink = WrappedSink::new(writer, YrsCodec::default());
  let sync_plugin = SyncPlugin::new(collab.clone(), sink, stream);
  collab.lock().collab.add_plugin(Arc::new(sync_plugin));
  collab.initial();

  Ok(collab)
}
