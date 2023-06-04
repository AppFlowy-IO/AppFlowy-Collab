use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use collab::core::collab::MutexCollab;
use collab::preclude::CollabPlugin;
use collab_persistence::doc::YrsDocAction;
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

pub struct CollabSnapshotPlugin {
  uid: i64,
  db: Arc<RocksCollabDB>,
  local_collab: Arc<MutexCollab>,
  /// the number of updates on disk when opening the document
  update_count: Arc<AtomicU32>,
  snapshot_per_update: u32,
  remove_updates_after_snapshot: bool,
  state: Arc<RwLock<GenSnapshotState>>,
}

impl CollabSnapshotPlugin {
  pub fn new(
    uid: i64,
    db: Arc<RocksCollabDB>,
    local_collab: Arc<MutexCollab>,
    snapshot_per_update: u32,
    remove_updates_after_snapshot: bool,
  ) -> Self {
    let state = Arc::new(RwLock::new(GenSnapshotState::Idle));
    let initial_update_count = Arc::new(AtomicU32::new(0));
    Self {
      uid,
      db,
      local_collab,
      update_count: initial_update_count,
      snapshot_per_update,
      remove_updates_after_snapshot,
      state,
    }
  }

  /// Return the snapshots for the given object id
  pub fn get_snapshots(&self, object_id: &str) -> Vec<CollabSnapshot> {
    let transaction = self.db.read_txn();
    transaction.get_snapshots(self.uid, object_id)
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
        let remove_updates_after_snapshot = self.remove_updates_after_snapshot;

        // We use a blocking task to generate the snapshot
        tokio::spawn(async move {
          let _ = tokio::task::spawn_blocking(move || {
            if let (Some(state), Some(local_collab), Some(db)) = (
              weak_state.upgrade(),
              weak_local_collab.upgrade(),
              weak_db.upgrade(),
            ) {
              let result = db.with_write_txn(|w_db_txn| {
                let update_key = w_db_txn
                  .get_doc_last_update_key(uid, &object_id)
                  .ok_or(PersistenceError::LatestUpdateKeyNotExist)?;

                // Create a new snapshot that contains all the document data.
                w_db_txn.push_snapshot(
                  uid,
                  &object_id,
                  update_key.as_ref(),
                  &local_collab.lock().transact(),
                )?;

                // Delete all the updates prior to the new update specified by the update key.
                if remove_updates_after_snapshot {
                  w_db_txn.delete_updates_to(uid, &object_id, update_key.as_ref())?;
                }
                Ok(())
              });

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
