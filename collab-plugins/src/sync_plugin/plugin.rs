use std::sync::Arc;

use collab::core::collab_awareness::MutexCollabAwareness;
use collab::preclude::CollabPlugin;
use collab_sync::client::Connection;
use collab_sync::error::SyncError;
use futures_util::{SinkExt, StreamExt};
use y_sync::sync::{Message, SyncMessage};
use yrs::updates::encoder::Encode;
use yrs::TransactionMut;

pub struct SyncPlugin<Sink, Stream> {
  conn: Arc<Connection<Sink, Stream>>,
}

impl<Sink, Stream> SyncPlugin<Sink, Stream> {
  pub fn new<E>(awareness: Arc<MutexCollabAwareness>, sink: Sink, stream: Stream) -> Self
  where
    E: Into<SyncError> + Send + Sync,
    Sink: SinkExt<Vec<u8>, Error = E> + Send + Sync + Unpin + 'static,
    Stream: StreamExt<Item = Result<Vec<u8>, E>> + Send + Sync + Unpin + 'static,
  {
    let conn = Arc::new(Connection::new(awareness, sink, stream));
    Self { conn }
  }
}

impl<E, Sink, Stream> CollabPlugin for SyncPlugin<Sink, Stream>
where
  E: Into<SyncError> + Send + Sync,
  Sink: SinkExt<Vec<u8>, Error = E> + Send + Sync + Unpin + 'static,
  Stream: StreamExt<Item = Result<Vec<u8>, E>> + Send + Sync + Unpin + 'static,
{
  fn did_receive_update(&self, _object_id: &str, _txn: &TransactionMut, update: &[u8]) {
    let weak_conn = Arc::downgrade(&self.conn);
    let update = update.to_vec();
    tokio::spawn(async move {
      if let Some(conn) = weak_conn.upgrade() {
        let update = Message::Sync(SyncMessage::Update(update)).encode_v1();
        conn.send(update).await.unwrap();
      }
    });
  }
}
