use collab::lock::RwLock;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Weak};
use std::time::Duration;

use tokio_retry::strategy::FibonacciBackoff;
use tokio_retry::{Action, Retry};
use tokio_stream::StreamExt;
use tokio_stream::wrappers::WatchStream;

use collab::core::collab_plugin::CollabPluginType;
use collab::core::origin::CollabOrigin;
use collab::preclude::{Collab, CollabPlugin};
use collab_entity::CollabObject;

use crate::CollabKVDB;
use crate::cloud_storage::remote_collab::{RemoteCollab, RemoteCollabStorage};
use crate::cloud_storage::sink::{SinkConfig, SinkStrategy};

pub struct SupabaseDBPlugin {
  uid: i64,
  object: CollabObject,
  local_collab: Weak<RwLock<Collab>>,
  local_collab_storage: Weak<CollabKVDB>,
  remote_collab: Arc<RemoteCollab>,
  remote_collab_storage: Arc<dyn RemoteCollabStorage>,
  pending_updates: Arc<RwLock<Vec<Vec<u8>>>>,
  is_first_sync_done: Arc<AtomicBool>,
}

impl SupabaseDBPlugin {
  pub fn new(
    uid: i64,
    object: CollabObject,
    local_collab: Weak<RwLock<Collab>>,
    sync_per_secs: u64,
    remote_collab_storage: Arc<dyn RemoteCollabStorage>,
    local_collab_storage: Weak<CollabKVDB>,
  ) -> Self {
    let pending_updates = Arc::new(RwLock::from(Vec::new()));
    let is_first_sync_done = Arc::new(AtomicBool::new(false));

    let config = SinkConfig::new()
      .with_timeout(10)
      .with_strategy(SinkStrategy::FixInterval(Duration::from_secs(
        sync_per_secs,
      )));
    let remote_collab = Arc::new(RemoteCollab::new(
      object.clone(),
      remote_collab_storage.clone(),
      config,
      local_collab.clone(),
    ));

    // Subscribe the sync state from the remote collab
    let remote_sync_state = remote_collab.subscribe_sync_state();
    let mut remote_sync_state_stream = WatchStream::new(remote_sync_state);
    let weak_local_collab = local_collab.clone();
    tokio::spawn(async move {
      while let Some(new_state) = remote_sync_state_stream.next().await {
        if let Some(local_collab) = weak_local_collab.upgrade() {
          local_collab.read().await.set_sync_state(new_state);
        }
      }
    });

    Self {
      uid,
      object,
      local_collab,
      remote_collab,
      pending_updates,
      is_first_sync_done,
      local_collab_storage,
      remote_collab_storage,
    }
  }
}

impl CollabPlugin for SupabaseDBPlugin {
  fn did_init(&self, _collab: &Collab, _object_id: &str) {
    // TODO(nathan): retry action might take a long time even if the network is ready or enable of
    // the [RemoteCollabStorage] is true
    let retry_strategy = FibonacciBackoff::from_millis(2000);
    let action = InitSyncAction {
      uid: self.uid,
      object: self.object.clone(),
      remote_collab: Arc::downgrade(&self.remote_collab),
      local_collab: self.local_collab.clone(),
      local_collab_storage: self.local_collab_storage.clone(),
      remote_collab_storage: Arc::downgrade(&self.remote_collab_storage),
      pending_updates: Arc::downgrade(&self.pending_updates),
      is_first_sync_done: Arc::downgrade(&self.is_first_sync_done),
    };

    tokio::spawn(async move {
      let _ = Retry::spawn(retry_strategy, action).await;
    });
  }

  fn receive_local_update(&self, origin: &CollabOrigin, object_id: &str, update: &[u8]) {
    if self.is_first_sync_done.load(Ordering::SeqCst) {
      if let Err(e) = self.remote_collab.push_update(update) {
        tracing::error!(
          "Collab {} failed to apply update from {}: {}",
          object_id,
          origin,
          e
        );
      };
    } else {
      self.pending_updates.blocking_write().push(update.to_vec());
    }
  }

  fn plugin_type(&self) -> CollabPluginType {
    CollabPluginType::CloudStorage
  }
}

#[allow(dead_code)]
struct InitSyncAction {
  uid: i64,
  object: CollabObject,
  remote_collab: Weak<RemoteCollab>,
  local_collab: Weak<RwLock<Collab>>,
  local_collab_storage: Weak<CollabKVDB>,
  remote_collab_storage: Weak<dyn RemoteCollabStorage>,
  pending_updates: Weak<RwLock<Vec<Vec<u8>>>>,
  is_first_sync_done: Weak<AtomicBool>,
}

impl Action for InitSyncAction {
  type Future = Pin<Box<dyn Future<Output = Result<Self::Item, Self::Error>> + Send>>;
  type Item = ();
  type Error = anyhow::Error;

  fn run(&mut self) -> Self::Future {
    let weak_remote_collab = self.remote_collab.clone();
    let weak_pending_updates = self.pending_updates.clone();
    let weak_is_first_sync_done = self.is_first_sync_done.clone();

    Box::pin(async move {
      if let (Some(remote_collab), Some(pending_updates), Some(is_first_sync_done)) = (
        weak_remote_collab.upgrade(),
        weak_pending_updates.upgrade(),
        weak_is_first_sync_done.upgrade(),
      ) {
        for update in &*pending_updates.read().await {
          remote_collab.push_update(update)?;
        }

        is_first_sync_done.store(true, Ordering::SeqCst);
        Ok(())
      } else {
        Ok(())
      }
    })
  }
}
