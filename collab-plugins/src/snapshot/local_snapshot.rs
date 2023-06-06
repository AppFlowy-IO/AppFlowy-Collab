use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use collab::core::collab::MutexCollab;
use collab::preclude::CollabPlugin;

use collab_persistence::kv::rocks_kv::RocksCollabDB;
use collab_persistence::snapshot::{CollabSnapshot, SnapshotAction};
use collab_persistence::PersistenceError;
use parking_lot::RwLock;
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

pub trait SnapshotDB: Send + Sync {
  fn get_snapshots(&self, uid: i64, object_id: &str) -> Vec<CollabSnapshot>;

  fn create_snapshot(
    &self,
    uid: i64,
    object_id: &str,
    collab: Arc<MutexCollab>,
  ) -> Result<(), PersistenceError>;
}

pub struct CollabSnapshotPlugin {
  uid: i64,
  db: Arc<dyn SnapshotDB>,
  local_collab: Arc<MutexCollab>,
  /// the number of updates on disk when opening the document
  update_count: Arc<AtomicU32>,
  snapshot_per_update: u32,
  state: Arc<RwLock<GenSnapshotState>>,
}

impl CollabSnapshotPlugin {
  pub fn new(
    uid: i64,
    db: Arc<dyn SnapshotDB>,
    local_collab: Arc<MutexCollab>,
    snapshot_per_update: u32,
  ) -> Self {
    let state = Arc::new(RwLock::new(GenSnapshotState::Idle));
    let initial_update_count = Arc::new(AtomicU32::new(0));
    Self {
      uid,
      db,
      local_collab,
      update_count: initial_update_count,
      snapshot_per_update,
      state,
    }
  }

  /// Return the snapshots for the given object id
  pub fn get_snapshots(&self, object_id: &str) -> Vec<CollabSnapshot> {
    self.db.get_snapshots(self.uid, object_id)
  }
}

impl CollabPlugin for CollabSnapshotPlugin {
  fn after_transaction(&self, object_id: &str, _txn: &mut TransactionMut) {
    // After each transaction, we increment the update count
    let old_value = self.update_count.fetch_add(1, Ordering::SeqCst);

    // If the number of updates is greater than the threshold, we generate a snapshot
    // and push it to the database
    if old_value != 0 && (old_value + 1) % self.snapshot_per_update == 0 {
      let is_idle = self.state.read().is_idle();
      if is_idle {
        *self.state.write() = GenSnapshotState::Processing;
        let weak_local_collab = Arc::downgrade(&self.local_collab);
        let weak_state = Arc::downgrade(&self.state);
        let weak_db = Arc::downgrade(&self.db);
        let uid = self.uid;
        let object_id = object_id.to_string();

        // We use a blocking task to generate the snapshot
        tokio::spawn(async move {
          let _ = tokio::task::spawn_blocking(move || {
            if let (Some(state), Some(local_collab), Some(db)) = (
              weak_state.upgrade(),
              weak_local_collab.upgrade(),
              weak_db.upgrade(),
            ) {
              // Create a new snapshot that contains all the document data.
              let result = db.create_snapshot(uid, &object_id, local_collab);
              match result {
                Ok(_) => tracing::trace!("{} snapshot generated", object_id),
                Err(e) => tracing::error!("{} snapshot generation failed: {}", object_id, e),
              }
              *state.write() = GenSnapshotState::Idle;
            };
            Ok::<(), PersistenceError>(())
          })
          .await;
        });
      }
    }
  }
}

impl SnapshotDB for Arc<RocksCollabDB> {
  fn get_snapshots(&self, uid: i64, object_id: &str) -> Vec<CollabSnapshot> {
    self.read_txn().get_snapshots(uid, object_id)
  }

  fn create_snapshot(
    &self,
    uid: i64,
    object_id: &str,
    collab: Arc<MutexCollab>,
  ) -> Result<(), PersistenceError> {
    self.with_write_txn(|txn| {
      txn.push_snapshot(uid, object_id, &collab.lock().transact())?;
      Ok(())
    })
  }
}
