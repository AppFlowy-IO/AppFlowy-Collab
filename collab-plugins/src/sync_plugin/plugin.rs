use std::sync::Arc;

use collab::core::collab::CollabOrigin;
use collab::core::collab_awareness::MutexCollabAwareness;
use collab::preclude::CollabPlugin;
use collab_sync::client::sync::SyncQueue;

use collab_sync::error::SyncError;
use collab_sync::msg::{ClientUpdateMessage, CollabMessage};
use futures_util::{SinkExt, StreamExt};
use y_sync::sync::{Message, SyncMessage};
use yrs::updates::encoder::Encode;

pub struct SyncPlugin<Sink, Stream> {
  object_id: String,
  sync_queue: Arc<SyncQueue<Sink, Stream>>,
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
    E: Into<SyncError> + Send + Sync + 'static,
    Sink: SinkExt<CollabMessage, Error = E> + Send + Sync + Unpin + 'static,
    Stream: StreamExt<Item = Result<CollabMessage, E>> + Send + Sync + Unpin + 'static,
  {
    let sync_queue = Arc::new(SyncQueue::new(object_id, origin, sink, stream, awareness));
    Self {
      sync_queue,
      object_id: object_id.to_string(),
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
    let weak_sync_queue = Arc::downgrade(&self.sync_queue);
    let update = update.to_vec();
    let object_id = self.object_id.clone();
    let cloned_origin = origin.clone();

    tokio::spawn(async move {
      if let Some(sync_queue) = weak_sync_queue.upgrade() {
        let payload = Message::Sync(SyncMessage::Update(update)).encode_v1();
        sync_queue
          .sync_msg(|msg_id| {
            ClientUpdateMessage::new(cloned_origin, object_id, msg_id, payload).into()
          })
          .await;
      }
    });
  }
}
