use std::sync::Arc;

use collab::core::collab_awareness::MutexCollabAwareness;
use collab::preclude::CollabPlugin;
use collab_sync::client::ClientSync;
use collab_sync::error::SyncError;
use collab_sync::message::{CollabClientMessage, CollabMessage};
use futures_util::{SinkExt, StreamExt};
use y_sync::sync::{Message, SyncMessage};
use yrs::updates::encoder::Encode;
use yrs::TransactionMut;

pub struct SyncPlugin<Sink, Stream> {
  uid: i64,
  object_id: String,
  client_sync: Arc<ClientSync<Sink, Stream>>,
}

impl<Sink, Stream> SyncPlugin<Sink, Stream> {
  pub fn new<E>(
    uid: i64,
    object_id: &str,
    awareness: Arc<MutexCollabAwareness>,
    sink: Sink,
    stream: Stream,
  ) -> Self
  where
    E: Into<SyncError> + Send + Sync,
    Sink: SinkExt<CollabMessage, Error = E> + Send + Sync + Unpin + 'static,
    Stream: StreamExt<Item = Result<CollabMessage, E>> + Send + Sync + Unpin + 'static,
  {
    let client_sync = Arc::new(ClientSync::new(uid, object_id, awareness, sink, stream));
    let doc_id = object_id.to_string();
    Self {
      uid,
      client_sync,
      object_id: doc_id,
    }
  }
}

impl<E, Sink, Stream> CollabPlugin for SyncPlugin<Sink, Stream>
where
  E: Into<SyncError> + Send + Sync,
  Sink: SinkExt<CollabMessage, Error = E> + Send + Sync + Unpin + 'static,
  Stream: StreamExt<Item = Result<CollabMessage, E>> + Send + Sync + Unpin + 'static,
{
  fn did_receive_update(&self, _object_id: &str, _txn: &TransactionMut, update: &[u8]) {
    let weak_client_sync = Arc::downgrade(&self.client_sync);
    let update = update.to_vec();
    let object_id = self.object_id.clone();
    let from_uid = self.uid;
    tokio::spawn(async move {
      if let Some(weak_client_sync) = weak_client_sync.upgrade() {
        let payload = Message::Sync(SyncMessage::Update(update)).encode_v1();
        let msg: CollabMessage = CollabClientMessage::new(from_uid, object_id, payload).into();
        weak_client_sync.send(msg).await.unwrap();
      }
    });
  }
}
