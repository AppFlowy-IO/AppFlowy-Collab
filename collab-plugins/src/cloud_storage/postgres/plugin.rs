use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Weak};
use std::time::Duration;

use collab::core::collab::MutexCollab;
use collab::core::collab_plugin::CollabPluginType;
use collab::core::collab_state::SnapshotState;
use collab::core::origin::CollabOrigin;
use collab::preclude::{Collab, CollabPlugin};
use collab_persistence::doc::YrsDocAction;
use collab_persistence::kv::rocks_kv::RocksCollabDB;
use collab_persistence::TransactionMutExt;
use collab_sync::client::sink::{SinkConfig, SinkStrategy};
use parking_lot::RwLock;
use tokio_retry::strategy::FibonacciBackoff;
use tokio_retry::{Action, Retry};
use tokio_stream::wrappers::WatchStream;
use tokio_stream::StreamExt;
use y_sync::awareness::Awareness;
use yrs::updates::decoder::Decode;
use yrs::{ReadTxn, StateVector, Update};

use crate::cloud_storage::remote_collab::{
  should_create_snapshot, CollabObject, RemoteCollab, RemoteCollabStorage,
};

pub struct SupabaseDBPlugin {
  uid: i64,
  object: CollabObject,
  local_collab: Weak<MutexCollab>,
  local_collab_storage: Arc<RocksCollabDB>,
  remote_collab: Arc<RemoteCollab>,
  remote_collab_storage: Arc<dyn RemoteCollabStorage>,
  pending_updates: Arc<RwLock<Vec<Vec<u8>>>>,
  is_first_sync_done: Arc<AtomicBool>,
}

impl SupabaseDBPlugin {
  pub fn new(
    uid: i64,
    object: CollabObject,
    local_collab: Weak<MutexCollab>,
    sync_per_secs: u64,
    remote_collab_storage: Arc<dyn RemoteCollabStorage>,
    local_collab_storage: Arc<RocksCollabDB>,
  ) -> Self {
    let pending_updates = Arc::new(RwLock::new(Vec::new()));
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
    ));

    // Subscribe the sync state from the remote collab
    let remote_sync_state = remote_collab.subscribe_sync_state();
    let mut remote_sync_state_stream = WatchStream::new(remote_sync_state);
    let weak_local_collab = local_collab.clone();
    tokio::spawn(async move {
      while let Some(new_state) = remote_sync_state_stream.next().await {
        if let Some(local_collab) = weak_local_collab.upgrade() {
          local_collab.lock().set_sync_state(new_state);
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
  fn did_init(&self, _awareness: &Awareness, _object_id: &str) {
    // TODO(nathan): retry action might take a long time even if the network is ready or enable of
    // the [RemoteCollabStorage] is true
    let retry_strategy = FibonacciBackoff::from_millis(2000);
    let action = InitSyncAction {
      uid: self.uid,
      object: self.object.clone(),
      remote_collab: Arc::downgrade(&self.remote_collab),
      local_collab: self.local_collab.clone(),
      local_collab_storage: Arc::downgrade(&self.local_collab_storage),
      remote_collab_storage: Arc::downgrade(&self.remote_collab_storage),
      pending_updates: Arc::downgrade(&self.pending_updates),
      is_first_sync_done: Arc::downgrade(&self.is_first_sync_done),
    };

    tokio::spawn(async move {
      let _ = Retry::spawn(retry_strategy, action).await;
    });
  }

  fn receive_local_update(&self, _origin: &CollabOrigin, _object_id: &str, update: &[u8]) {
    if self.is_first_sync_done.load(Ordering::SeqCst) {
      self.remote_collab.push_update(update);
    } else {
      self.pending_updates.write().push(update.to_vec());
    }
  }

  fn plugin_type(&self) -> CollabPluginType {
    CollabPluginType::CloudStorage
  }

  fn reset(&self, _object_id: &str) {
    self.pending_updates.write().clear();
    self.remote_collab.clear();
  }
}

/// Create a snapshot for the object if need
/// If the remote_update is empty which means the object is not sync. So crate a snapshot for it.
/// If the remote_update is not empty, check the [RemoteCollabState] to decide whether create a snapshot.
fn create_snapshot_if_need(
  uid: i64,
  object: CollabObject,
  remote_update: Vec<u8>,
  weak_local_collab: Weak<MutexCollab>,
  weak_local_collab_storage: Weak<RocksCollabDB>,
  weak_remote_collab_storage: Weak<dyn RemoteCollabStorage>,
) {
  tokio::spawn(async move {
    if let (Some(local_collab_storage), Some(remote_collab_storage)) = (
      weak_local_collab_storage.upgrade(),
      weak_remote_collab_storage.upgrade(),
    ) {
      match remote_collab_storage.get_collab_state(&object.id).await {
        Ok(Some(collab_state)) => {
          if !should_create_snapshot(&collab_state) {
            return;
          }
        },
        Err(e) => {
          tracing::error!("🔴fetch remote collab state failed: {:?}", e);
          return;
        },
        _ => {
          // Create a snapshot if the remote state is empty
        },
      }

      tracing::trace!("Create remote snapshot for {}", object.id);
      let cloned_object = object.clone();
      if let Ok(Ok(doc_state)) = tokio::task::spawn_blocking(move || {
        let local = Collab::new(uid, object.id.clone(), vec![]);
        let mut txn = local.transact_mut();
        let _ = local_collab_storage
          .read_txn()
          .load_doc(uid, &object.id, &mut txn)?;
        drop(txn);

        // Only sync with the remote if the remote update is not empty
        if !remote_update.is_empty() {
          let remote = Collab::new(uid, object.id.clone(), vec![]);
          let mut txn = local.transact_mut();
          txn.try_apply_update(Update::decode_v1(&remote_update)?)?;
          drop(txn);

          let local_sv = local.transact().state_vector();
          let encode_update = remote.transact().encode_state_as_update_v1(&local_sv);
          if let Ok(update) = Update::decode_v1(&encode_update) {
            let mut txn = local.transact_mut();
            txn.try_apply_update(update)?;
            drop(txn);
          }
        }

        let txn = local.transact();
        Ok::<Vec<_>, anyhow::Error>(txn.encode_state_as_update_v1(&StateVector::default()))
      })
      .await
      {
        // Send the full sync data to remote
        match remote_collab_storage
          .create_snapshot(&cloned_object, doc_state)
          .await
        {
          Ok(snapshot_id) => {
            tracing::debug!("{} remote snapshot created", cloned_object.id);
            if let Some(local_collab) = weak_local_collab.upgrade() {
              local_collab
                .lock()
                .set_snapshot_state(SnapshotState::DidCreateSnapshot { snapshot_id });
            }
          },
          Err(e) => tracing::error!("🔴{}", e),
        }
      }
    }
  });
}

struct InitSyncAction {
  uid: i64,
  object: CollabObject,
  remote_collab: Weak<RemoteCollab>,
  local_collab: Weak<MutexCollab>,
  local_collab_storage: Weak<RocksCollabDB>,
  remote_collab_storage: Weak<dyn RemoteCollabStorage>,
  pending_updates: Weak<RwLock<Vec<Vec<u8>>>>,
  is_first_sync_done: Weak<AtomicBool>,
}

impl Action for InitSyncAction {
  type Future = Pin<Box<dyn Future<Output = Result<Self::Item, Self::Error>> + Send>>;
  type Item = ();
  type Error = anyhow::Error;

  fn run(&mut self) -> Self::Future {
    let uid = self.uid;
    let object = self.object.clone();
    let weak_remote_collab = self.remote_collab.clone();
    let weak_local_collab = self.local_collab.clone();
    let weak_local_collab_storage = self.local_collab_storage.clone();
    let weak_remote_collab_storage = self.remote_collab_storage.clone();
    let weak_pending_updates = self.pending_updates.clone();
    let weak_is_first_sync_done = self.is_first_sync_done.clone();

    Box::pin(async move {
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
        let remote_update = remote_collab.sync(local_collab.clone()).await?;

        create_snapshot_if_need(
          uid,
          object,
          remote_update,
          Arc::downgrade(&local_collab),
          weak_local_collab_storage,
          weak_remote_collab_storage,
        );

        for update in &*pending_updates.read() {
          remote_collab.push_update(update);
        }

        is_first_sync_done.store(true, Ordering::SeqCst);
        Ok(())
      } else {
        Ok(())
      }
    })
  }
}
