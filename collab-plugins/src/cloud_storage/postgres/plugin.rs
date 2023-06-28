use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use collab::core::collab::MutexCollab;
use collab::core::collab_plugin::CollabPluginType;
use collab::core::origin::CollabOrigin;
use collab::preclude::CollabPlugin;
use collab_sync::client::sink::{SinkConfig, SinkStrategy};
use parking_lot::RwLock;
use y_sync::awareness::Awareness;
use yrs::Transaction;

use tokio_stream::wrappers::WatchStream;
use tokio_stream::StreamExt;

use crate::cloud_storage::remote_collab::{CollabObject, RemoteCollab, RemoteCollabStorage};

pub struct SupabaseDBPlugin {
  local_collab: Arc<MutexCollab>,
  remote_collab: Arc<RemoteCollab>,
  pending_updates: Arc<RwLock<Vec<Vec<u8>>>>,
  is_first_sync_done: Arc<AtomicBool>,
}

impl SupabaseDBPlugin {
  pub fn new(
    object: CollabObject,
    local_collab: Arc<MutexCollab>,
    sync_per_secs: u64,
    storage: Arc<dyn RemoteCollabStorage>,
  ) -> Self {
    let pending_updates = Arc::new(RwLock::new(Vec::new()));
    let is_first_sync_done = Arc::new(AtomicBool::new(false));

    let config = SinkConfig::new()
      .with_timeout(15)
      .with_strategy(SinkStrategy::FixInterval(Duration::from_secs(
        sync_per_secs,
      )));
    let remote_collab = Arc::new(RemoteCollab::new(object, storage, config));

    // Subscribe the sync state from the remote collab
    let receiver = remote_collab.subscribe_sync_state();
    let mut receiver_stream = WatchStream::new(receiver);
    let weak_local_collab = Arc::downgrade(&local_collab);
    tokio::spawn(async move {
      while let Some(new_state) = receiver_stream.next().await {
        if let Some(local_collab) = weak_local_collab.upgrade() {
          local_collab.lock().set_sync_state(new_state);
        }
      }
    });

    Self {
      local_collab,
      remote_collab,
      pending_updates,
      is_first_sync_done,
    }
  }
}

impl CollabPlugin for SupabaseDBPlugin {
  fn did_init(&self, _awareness: &Awareness, _object_id: &str, _txn: &Transaction) {
    let weak_remote_collab = Arc::downgrade(&self.remote_collab);
    let weak_local_collab = Arc::downgrade(&self.local_collab);
    let weak_pending_updates = Arc::downgrade(&self.pending_updates);
    let weak_is_first_sync_done = Arc::downgrade(&self.is_first_sync_done);

    tokio::spawn(async move {
      if let (
        Some(remote_collab),
        Some(local_collab),
        Some(pending_updates),
        Some(is_first_sync_done),
      ) = (
        weak_remote_collab.upgrade(),
        weak_local_collab.upgrade(),
        weak_pending_updates.upgrade(),
        weak_is_first_sync_done.upgrade(),
      ) {
        remote_collab.sync(local_collab.clone()).await;
        for update in &*pending_updates.read() {
          remote_collab.push_update(update);
        }

        is_first_sync_done.store(true, Ordering::SeqCst)
      }
    });
  }

  fn receive_local_update(&self, _origin: &CollabOrigin, _object_id: &str, update: &[u8]) {
    tracing::trace!("Receive local update: {}", update.len());
    if self.is_first_sync_done.load(Ordering::SeqCst) {
      self.remote_collab.push_update(update);
    } else {
      self.pending_updates.write().push(update.to_vec());
    }
  }

  fn plugin_type(&self) -> CollabPluginType {
    CollabPluginType::CloudStorage
  }
}
