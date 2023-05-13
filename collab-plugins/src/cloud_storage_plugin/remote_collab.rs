use async_trait::async_trait;
use collab::core::collab::MutexCollab;
use collab_sync::client::sync::{SyncSink, TaskRunner};
use collab_sync::client::TokioUnboundedSink;
use std::cmp::Ordering;
use std::fmt::{Display, Formatter};

use collab::core::origin::CollabOrigin;
use std::sync::Arc;
use std::time::Duration;
use tokio::spawn;
use tokio::sync::mpsc::unbounded_channel;
use tokio::sync::watch;

use yrs::updates::decoder::Decode;
use yrs::{ReadTxn, Update};

#[async_trait]
pub trait RemoteCollabStorage: Send + Sync + 'static {
  async fn get_all_updates(&self, object_id: &str) -> Result<Vec<Vec<u8>>, anyhow::Error>;
  async fn send_update(&self, id: u32, update: Vec<u8>) -> Result<(), anyhow::Error>;
}

pub struct RemoteCollab {
  object_id: String,
  inner: Arc<MutexCollab>,
  storage: Arc<dyn RemoteCollabStorage>,
  sink: Arc<SyncSink<TokioUnboundedSink<Message>, Message>>,
}

impl RemoteCollab {
  /// Create a new remote collab.
  /// `timeout` is the time to wait for the server to ack the message.
  /// If the server does not ack the message in time, the message will be sent again.
  pub fn new<S>(object_id: String, storage: S, timeout: u64) -> Self
  where
    S: RemoteCollabStorage + Send + Sync + 'static,
  {
    let storage: Arc<dyn RemoteCollabStorage> = Arc::new(storage);
    let inner = Arc::new(MutexCollab::new(CollabOrigin::Empty, &object_id, vec![]));
    let (sink, mut stream) = unbounded_channel::<Message>();
    let weak_storage = Arc::downgrade(&storage);
    let (notifier, notifier_rx) = watch::channel(false);
    let sink = Arc::new(SyncSink::new(
      TokioUnboundedSink(sink),
      notifier,
      Duration::from_secs(timeout),
    ));

    let weak_sink = Arc::downgrade(&sink);
    spawn(async move {
      while let Some(update) = stream.recv().await {
        if let Some(storage) = weak_storage.upgrade() {
          tracing::trace!("send update: {}", update.msg_id);
          match storage.send_update(update.msg_id, update.payload).await {
            Ok(_) => {
              tracing::trace!("ack update: {}", update.msg_id);
              if let Some(sink) = weak_sink.upgrade() {
                sink.ack_msg(update.msg_id).await;
              }
            },
            Err(e) => {
              tracing::error!("send {} update failed: {:?}", update.msg_id, e);
            },
          }
        }
      }
    });

    spawn(TaskRunner::run(Arc::downgrade(&sink), notifier_rx));
    Self {
      object_id,
      inner,
      storage,
      sink,
    }
  }

  pub async fn sync(&self, local_collab: Arc<MutexCollab>) {
    let updates = self
      .storage
      .get_all_updates(&self.object_id)
      .await
      .unwrap_or_default();
    if !updates.is_empty() {
      self.inner.lock().with_transact_mut(|txn| {
        for update in updates {
          if let Ok(update) = Update::decode_v1(&update) {
            txn.apply_update(update);
          } else {
            tracing::error!("Failed to decode update");
          }
        }
      });

      // Update local collab
      let local_sv = local_collab.lock().transact().state_vector();
      let update = self
        .inner
        .lock()
        .transact()
        .encode_state_as_update_v1(&local_sv);
      if let Ok(update) = Update::decode_v1(&update) {
        local_collab.lock().with_transact_mut(|txn| {
          txn.apply_update(update);
        });
      }
    }

    // Update remote collab
    let remote_state_vector = self.inner.lock().transact().state_vector();
    let update = local_collab
      .lock()
      .transact()
      .encode_state_as_update_v1(&remote_state_vector);
    if let Ok(update) = Update::decode_v1(&update) {
      self.inner.lock().with_transact_mut(|txn| {
        txn.apply_update(update);
      });
    }
  }

  pub fn push_update(&self, update: Vec<u8>) {
    self.sink.queue_msg(|msg_id| Message {
      msg_id,
      payload: update,
    });
  }
}

#[derive(Clone, Debug)]
struct Message {
  msg_id: u32,
  payload: Vec<u8>,
}

impl Eq for Message {}

impl PartialEq for Message {
  fn eq(&self, other: &Self) -> bool {
    self.msg_id == other.msg_id
  }
}

impl PartialOrd for Message {
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
    Some(self.cmp(other))
  }
}

impl Ord for Message {
  fn cmp(&self, other: &Self) -> Ordering {
    self.msg_id.cmp(&other.msg_id).reverse()
  }
}

impl Display for Message {
  fn fmt(&self, _f: &mut Formatter<'_>) -> std::fmt::Result {
    Ok(())
  }
}

#[derive(Debug, thiserror::Error)]
enum CollabError {
  #[error("Internal error")]
  Internal(#[from] anyhow::Error),
}
