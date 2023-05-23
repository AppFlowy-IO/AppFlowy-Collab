use crate::disk::rocksdb::CollabPersistenceConfig;
use collab::core::collab::MutexCollab;
use collab::preclude::CollabPlugin;

use collab_persistence::kv::rocks_kv::RocksCollabDB;
use parking_lot::RwLock;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use yrs::TransactionMut;

enum GenSnapshotState {
  Idle,
  Processing,
}

impl GenSnapshotState {
  fn is_idle(&self) -> bool {
    matches!(self, Self::Idle)
  }
}

pub struct CollabSnapshotPlugin {
  db: Arc<RocksCollabDB>,
  local_collab: Arc<MutexCollab>,
  /// the number of updates on disk when opening the document
  update_count: Arc<AtomicU32>,
  config: CollabPersistenceConfig,
  state: Arc<RwLock<GenSnapshotState>>,
}

impl CollabSnapshotPlugin {
  pub fn new(
    db: Arc<RocksCollabDB>,
    local_collab: Arc<MutexCollab>,
    config: CollabPersistenceConfig,
  ) -> Self {
    let state = Arc::new(RwLock::new(GenSnapshotState::Idle));
    let initial_update_count = Arc::new(AtomicU32::new(0));
    Self {
      db,
      local_collab,
      update_count: initial_update_count,
      config,
      state,
    }
  }
}

impl CollabPlugin for CollabSnapshotPlugin {
  fn after_transaction(&self, _object_id: &str, _txn: &mut TransactionMut) {
    let old_value = self.update_count.fetch_add(1, Ordering::SeqCst);
    if old_value > self.config.snapshot_per_update {
      let is_idle = self.state.read().is_idle();
      if is_idle {
        *self.state.write() = GenSnapshotState::Processing;

        let weak_update_count = Arc::downgrade(&self.update_count);
        let weak_local_collab = Arc::downgrade(&self.local_collab);
        let weak_state = Arc::downgrade(&self.state);
        tokio::spawn(async move {
          if let (Some(state), Some(_local_collab), Some(update_count)) = (
            weak_state.upgrade(),
            weak_local_collab.upgrade(),
            weak_update_count.upgrade(),
          ) {
            // Generate snapshot

            *state.write() = GenSnapshotState::Idle;
            update_count.store(0, Ordering::SeqCst);
          };
        });
      }
    }
  }
}
