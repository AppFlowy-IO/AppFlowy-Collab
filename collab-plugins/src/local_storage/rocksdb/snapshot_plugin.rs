use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Weak};

use crate::local_storage::kv::doc::CollabKVAction;
use crate::local_storage::kv::snapshot::{CollabSnapshot, SnapshotPersistence};
use crate::local_storage::kv::PersistenceError;
use crate::CollabKVDB;
use collab::preclude::{Collab, CollabPlugin};
use collab_entity::CollabObject;
use parking_lot::RwLock;

use yrs::{ReadTxn, StateVector, TransactionMut};

#[derive(Clone, Debug)]
enum GenSnapshotState {
  Idle,
  Processing,
  Fail,
}

impl GenSnapshotState {
  fn is_idle(&self) -> bool {
    matches!(self, Self::Idle)
  }

  fn is_fail(&self) -> bool {
    matches!(self, Self::Fail)
  }
}

pub struct CollabSnapshotPlugin {
  uid: i64,
  object: CollabObject,
  collab_db: Weak<CollabKVDB>,
  /// the number of updates on disk when opening the document
  update_count: Arc<AtomicU32>,
  snapshot_per_update: u32,
  state: Arc<RwLock<GenSnapshotState>>,
  snapshot_persistence: Arc<dyn SnapshotPersistence>,
}

impl CollabSnapshotPlugin {
  pub fn new(
    uid: i64,
    object: CollabObject,
    snapshot_persistence: Arc<dyn SnapshotPersistence>,
    collab_db: Weak<CollabKVDB>,
    snapshot_per_update: u32,
  ) -> Self {
    let state = Arc::new(RwLock::new(GenSnapshotState::Idle));
    let initial_update_count = Arc::new(AtomicU32::new(0));
    Self {
      uid,
      snapshot_persistence,
      object,
      collab_db,
      update_count: initial_update_count,
      snapshot_per_update,
      state,
    }
  }

  /// Return the snapshots for the given object id
  pub fn get_snapshots(&self, object_id: &str) -> Vec<CollabSnapshot> {
    self.snapshot_persistence.get_snapshots(self.uid, object_id)
  }
}

impl CollabPlugin for CollabSnapshotPlugin {
  fn after_transaction(&self, _object_id: &str, _txn: &mut TransactionMut) {
    // After each transaction, we increment the update count
    let old_value = self.update_count.fetch_add(1, Ordering::SeqCst);

    // If the number of updates is greater than the threshold, we generate a snapshot
    // and push it to the database. If the state is fail, which means the previous snapshot
    // generation failed, we try to generate a new snapshot again on the next transaction.
    let should_create_snapshot = old_value != 0 && (old_value + 1) % self.snapshot_per_update == 0;
    let state = self.state.read().clone();
    if should_create_snapshot || state.is_fail() {
      let is_ready = state.is_fail() || state.is_idle();
      if is_ready {
        *self.state.write() = GenSnapshotState::Processing;
        let weak_collab_db = self.collab_db.clone();
        let weak_state = Arc::downgrade(&self.state);
        let weak_snapshot_persistence = Arc::downgrade(&self.snapshot_persistence);
        let uid = self.uid;
        let object = self.object.clone();

        // We use a blocking task to generate the snapshot
        tokio::spawn(async move {
          let _ = tokio::task::spawn_blocking(move || {
            if let (Some(state), Some(collab_db), Some(snapshot_persistence)) = (
              weak_state.upgrade(),
              weak_collab_db.upgrade(),
              weak_snapshot_persistence.upgrade(),
            ) {
              let snapshot_collab = Collab::new(uid, object.object_id.clone(), "1", vec![]);
              let mut txn = snapshot_collab.origin_transact_mut();
              if let Err(e) =
                collab_db
                  .read_txn()
                  .load_doc_with_txn(uid, &object.object_id, &mut txn)
              {
                tracing::error!("{} snapshot generation failed: {}", object.object_id, e);
                *state.write() = GenSnapshotState::Fail;
                return Ok::<(), PersistenceError>(());
              }
              drop(txn);

              // Generate the snapshot
              let txn = snapshot_collab.transact();
              let encoded_v1 = txn.encode_state_as_update_v1(&StateVector::default());
              match snapshot_persistence.create_snapshot(
                uid,
                &object.object_id,
                &object.collab_type,
                encoded_v1,
              ) {
                Ok(_) => *state.write() = GenSnapshotState::Idle,
                Err(e) => {
                  tracing::error!("{} snapshot generation failed: {}", object.object_id, e);
                  *state.write() = GenSnapshotState::Fail;
                },
              }
            }
            Ok::<(), PersistenceError>(())
          })
          .await;
        });
      }
    }
  }
}