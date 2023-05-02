use std::sync::atomic::AtomicU32;
use std::sync::atomic::Ordering::SeqCst;
use std::sync::Arc;

use collab::core::collab::CollabOrigin;
use collab::core::collab_awareness::MutexCollabAwareness;
use collab::preclude::CollabPlugin;
use collab_sync::client::sync::ClientSync;
use collab_sync::error::SyncError;
use collab_sync::msg::{ClientUpdateMessage, CollabMessage};
use futures_util::{SinkExt, StreamExt};
use y_sync::sync::{Message, SyncMessage};
use yrs::updates::encoder::Encode;

pub struct SyncPlugin<Sink, Stream> {
  object_id: String,
  client_sync: Arc<ClientSync<Sink, Stream>>,
  msg_id_counter: Arc<AtomicU32>,
}

impl<Sink, Stream> SyncPlugin<Sink, Stream> {
  pub fn new<E>(
    origin: CollabOrigin,
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
    let msg_id_counter = Arc::new(AtomicU32::new(0));
    let client_sync = Arc::new(ClientSync::new(
      origin,
      object_id,
      msg_id_counter.clone(),
      awareness,
      sink,
      stream,
    ));
    Self {
      client_sync,
      object_id: object_id.to_string(),
      msg_id_counter,
    }
  }
}

impl<E, Sink, Stream> CollabPlugin for SyncPlugin<Sink, Stream>
where
  E: Into<SyncError> + Send + Sync,
  Sink: SinkExt<CollabMessage, Error = E> + Send + Sync + Unpin + 'static,
  Stream: StreamExt<Item = Result<CollabMessage, E>> + Send + Sync + Unpin + 'static,
{
  fn did_receive_local_update(&self, origin: &CollabOrigin, _object_id: &str, update: &[u8]) {
    let weak_client_sync = Arc::downgrade(&self.client_sync);
    let update = update.to_vec();
    let object_id = self.object_id.clone();
    let msg_id = self.msg_id_counter.fetch_add(1, SeqCst);
    let cloned_origin = origin.clone();

    tokio::spawn(async move {
      if let Some(weak_client_sync) = weak_client_sync.upgrade() {
        tracing::trace!(
          "[🦀Client]: [uid:{}|device_id:{}|msg_id:{}] send update",
          cloned_origin.uid,
          cloned_origin.device_id,
          msg_id,
        );
        let payload = Message::Sync(SyncMessage::Update(update)).encode_v1();
        let msg: CollabMessage =
          ClientUpdateMessage::new(cloned_origin, object_id, msg_id, payload).into();
        weak_client_sync.send(msg).await.unwrap();
      }
    });
  }
}
