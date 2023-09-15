use std::sync::{Arc, Weak};

use crate::sync_plugin::client::SinkConfig;
use crate::sync_plugin::client::SyncQueue;
use collab::core::collab::MutexCollab;
use collab::core::origin::CollabOrigin;
use collab::preclude::CollabPlugin;
use collab_define::CollabType;
use collab_sync_protocol::{ClientCollabUpdate, CollabMessage};
use futures_util::{SinkExt, StreamExt};
use y_sync::awareness::Awareness;
use y_sync::sync::{Message, SyncMessage};
use yrs::updates::encoder::Encode;

#[derive(Clone, Debug)]
pub struct SyncObject {
  pub object_id: String,
  pub workspace_id: String,
  pub collab_type: CollabType,
}

impl SyncObject {
  pub fn new(object_id: &str, workspace_id: &str, collab_type: CollabType) -> Self {
    Self {
      object_id: object_id.to_string(),
      workspace_id: workspace_id.to_string(),
      collab_type,
    }
  }
}

pub struct SyncPlugin<Sink, Stream> {
  object: SyncObject,
  sync_queue: Arc<SyncQueue<Sink, Stream>>,
}

impl<Sink, Stream> SyncPlugin<Sink, Stream> {
  pub fn new<E>(
    origin: CollabOrigin,
    object: SyncObject,
    collab: Weak<MutexCollab>,
    sink: Sink,
    stream: Stream,
  ) -> Self
  where
    E: std::error::Error + Send + Sync + 'static,
    Sink: SinkExt<CollabMessage, Error = E> + Send + Sync + Unpin + 'static,
    Stream: StreamExt<Item = Result<CollabMessage, E>> + Send + Sync + Unpin + 'static,
  {
    let sync_queue = SyncQueue::new(
      object.clone(),
      origin,
      sink,
      stream,
      collab,
      SinkConfig::default(),
    );
    Self {
      sync_queue: Arc::new(sync_queue),
      object,
    }
  }
}

impl<E, Sink, Stream> CollabPlugin for SyncPlugin<Sink, Stream>
where
  E: std::error::Error + Send + Sync + 'static,
  Sink: SinkExt<CollabMessage, Error = E> + Send + Sync + Unpin + 'static,
  Stream: StreamExt<Item = Result<CollabMessage, E>> + Send + Sync + Unpin + 'static,
{
  fn did_init(&self, _awareness: &Awareness, _object_id: &str) {
    self.sync_queue.notify(_awareness);
  }

  fn receive_local_update(&self, origin: &CollabOrigin, _object_id: &str, update: &[u8]) {
    let weak_sync_queue = Arc::downgrade(&self.sync_queue);
    let update = update.to_vec();
    let object_id = self.object.object_id.clone();
    let cloned_origin = origin.clone();

    tokio::spawn(async move {
      if let Some(sync_queue) = weak_sync_queue.upgrade() {
        let payload = Message::Sync(SyncMessage::Update(update)).encode_v1();
        sync_queue.queue_msg(|msg_id| {
          ClientCollabUpdate::new(cloned_origin, object_id, msg_id, payload).into()
        });
      }
    });
  }

  fn reset(&self, _object_id: &str) {
    self.sync_queue.clear();
  }
}
