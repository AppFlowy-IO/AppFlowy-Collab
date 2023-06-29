use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Weak};
use std::time::Duration;

use collab::core::collab::MutexCollab;
use collab::core::collab_plugin::CollabPluginType;
use collab::core::origin::CollabOrigin;
use collab::preclude::{Collab, CollabPlugin};
use collab_persistence::doc::YrsDocAction;
use collab_persistence::kv::rocks_kv::RocksCollabDB;
use collab_sync::client::sink::{SinkConfig, SinkStrategy};
use parking_lot::RwLock;
use tokio_stream::wrappers::WatchStream;
use tokio_stream::StreamExt;
use y_sync::awareness::Awareness;
use yrs::{ReadTxn, StateVector, Transaction, TransactionMut};

use crate::cloud_storage::remote_collab::{CollabObject, RemoteCollab, RemoteCollabStorage};

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
      .with_timeout(15)
      .with_strategy(SinkStrategy::FixInterval(Duration::from_secs(
        sync_per_secs,
      )));
    let remote_collab = Arc::new(RemoteCollab::new(
      object.clone(),
      remote_collab_storage.clone(),
      config,
    ));

    // Subscribe the sync state from the remote collab
    let receiver = remote_collab.subscribe_sync_state();
    let mut receiver_stream = WatchStream::new(receiver);
    let weak_local_collab = local_collab.clone();
    tokio::spawn(async move {
      while let Some(new_state) = receiver_stream.next().await {
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
  fn did_init(&self, _awareness: &Awareness, _object_id: &str, _txn: &Transaction) {
    let weak_remote_collab = Arc::downgrade(&self.remote_collab);
    let weak_local_collab = self.local_collab.clone();
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

  fn after_transaction(&self, _object_id: &str, _txn: &mut TransactionMut) {
    let uid = self.uid;
    let object = self.object.clone();
    let weak_local_collab_storage = Arc::downgrade(&self.local_collab_storage);
    let weak_remote_collab_storage = Arc::downgrade(&self.remote_collab_storage);

    // We use a blocking task to generate the snapshot
    tokio::spawn(async move {
      if let (Some(local_collab_storage), Some(remote_collab_storage)) = (
        weak_local_collab_storage.upgrade(),
        weak_remote_collab_storage.upgrade(),
      ) {
        let cloned_object = object.clone();
        if let Ok(Ok(full_sync_data)) = tokio::task::spawn_blocking(move || {
          let collab = Collab::new(uid, object.id.clone(), vec![]);
          let mut txn = collab.transact_mut();
          let _ = local_collab_storage
            .read_txn()
            .load_doc(uid, &object.id, &mut txn)?;
          drop(txn);

          // Generate the full sync data
          let txn = collab.transact();
          Ok::<Vec<_>, anyhow::Error>(txn.encode_state_as_update_v1(&StateVector::default()))
        })
        .await
        {
          // Send the full sync data to remote
          match remote_collab_storage
            .create_full_sync(&cloned_object, full_sync_data)
            .await
          {
            Ok(_) => tracing::debug!("{} full sync created", cloned_object.id),
            Err(e) => tracing::error!("Failed to create full sync: {}", e),
          }
        }
      }
    });
  }

  fn plugin_type(&self) -> CollabPluginType {
    CollabPluginType::CloudStorage
  }
}
