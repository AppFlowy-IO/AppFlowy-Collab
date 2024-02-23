use std::sync::{Arc, Weak};

use crate::local_storage::kv::doc::CollabKVAction;
use crate::local_storage::kv::snapshot::SnapshotPersistence;
use crate::local_storage::kv::{KVTransactionDB, PersistenceError};
use crate::CollabKVDB;
use collab::preclude::Collab;
use collab_entity::CollabType;
use parking_lot::RwLock;

use yrs::{ReadTxn, StateVector};

#[derive(Clone, Debug)]
pub(crate) enum SnapshotState {
  Idle,
  Processing,
  Fail,
}

impl SnapshotState {
  fn is_processing(&self) -> bool {
    matches!(self, Self::Processing)
  }
}

#[derive(Clone)]
pub struct CollabSnapshot {
  state: Arc<RwLock<SnapshotState>>,
  snapshot_persistence: Arc<dyn SnapshotPersistence>,
}

impl CollabSnapshot {
  pub fn new(snapshot_persistence: Arc<dyn SnapshotPersistence>) -> Self {
    let state = Arc::new(RwLock::new(SnapshotState::Idle));
    Self {
      snapshot_persistence,
      state,
    }
  }

  pub(crate) fn should_create_snapshot(&self) -> bool {
    if let Some(mut state) = self.state.try_write() {
      if !state.is_processing() {
        *state = SnapshotState::Processing;
        return true;
      }
    }
    false
  }

  pub(crate) fn create_snapshot(
    &self,
    weak_collab_db: Weak<CollabKVDB>,
    uid: i64,
    object_id: &str,
    collab_type: &CollabType,
  ) {
    let weak_state = Arc::downgrade(&self.state);
    let weak_snapshot_persistence = Arc::downgrade(&self.snapshot_persistence);
    let object_id = object_id.to_string();
    let collab_type = collab_type.clone();

    // We use a blocking task to generate the snapshot
    tokio::spawn(async move {
      let _ = tokio::task::spawn_blocking(move || {
        if let (Some(state), Some(collab_db), Some(snapshot_persistence)) = (
          weak_state.upgrade(),
          weak_collab_db.upgrade(),
          weak_snapshot_persistence.upgrade(),
        ) {
          let snapshot_collab = Collab::new(uid, object_id.clone(), "1", vec![], false);
          let mut txn = snapshot_collab.origin_transact_mut();
          if let Err(e) = collab_db
            .read_txn()
            .load_doc_with_txn(uid, &object_id, &mut txn)
          {
            tracing::error!("{} snapshot generation failed: {}", object_id, e);
            *state.write() = SnapshotState::Fail;
            return Ok::<(), PersistenceError>(());
          }
          drop(txn);

          // Generate the snapshot
          let txn = snapshot_collab.transact();
          let encoded_v1 = txn.encode_state_as_update_v1(&StateVector::default());
          match snapshot_persistence.create_snapshot(uid, &object_id, &collab_type, encoded_v1) {
            Ok(_) => *state.write() = SnapshotState::Idle,
            Err(e) => {
              tracing::error!("{} snapshot generation failed: {}", object_id, e);
              *state.write() = SnapshotState::Fail;
            },
          }
        }
        Ok::<(), PersistenceError>(())
      })
      .await;
    });
  }
}
