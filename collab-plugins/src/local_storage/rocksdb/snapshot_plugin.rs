use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Weak};

use crate::local_storage::kv::doc::CollabKVAction;
use crate::local_storage::kv::snapshot::SnapshotPersistence;
use crate::local_storage::kv::{KVTransactionDB, PersistenceError};
use crate::CollabKVDB;
use collab::preclude::Collab;
use collab_entity::CollabType;

use yrs::{ReadTxn, StateVector};

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SnapshotState {
  Idle = SnapshotState::IDLE,
  Processing = SnapshotState::PROCESSING,
  Fail = SnapshotState::FAIL,
}

impl SnapshotState {
  const IDLE: u8 = 0;
  const PROCESSING: u8 = 1;
  const FAIL: u8 = 2;
}

impl TryFrom<u8> for SnapshotState {
  type Error = u8;

  #[inline]
  fn try_from(value: u8) -> Result<Self, Self::Error> {
    match value {
      SnapshotState::IDLE => Ok(SnapshotState::Idle),
      SnapshotState::PROCESSING => Ok(SnapshotState::Processing),
      SnapshotState::FAIL => Ok(SnapshotState::Fail),
      _ => Err(value),
    }
  }
}

#[allow(dead_code)]
#[derive(Clone)]
pub struct CollabSnapshot {
  state: Arc<AtomicU8>,
  snapshot_persistence: Arc<dyn SnapshotPersistence>,
}

#[allow(dead_code)]
impl CollabSnapshot {
  pub fn new(snapshot_persistence: Arc<dyn SnapshotPersistence>) -> Self {
    let state = Arc::new(AtomicU8::new(SnapshotState::IDLE));
    Self {
      snapshot_persistence,
      state,
    }
  }

  #[inline]
  fn swap_state(&self, state: SnapshotState) -> SnapshotState {
    let old = self.state.swap(state as u8, Ordering::Release);
    SnapshotState::try_from(old).unwrap()
  }

  pub(crate) fn should_create_snapshot(&self) -> bool {
    let old = self.swap_state(SnapshotState::Processing);
    old != SnapshotState::Processing
  }

  pub(crate) fn create_snapshot(
    &self,
    weak_collab_db: Weak<CollabKVDB>,
    uid: i64,
    object_id: &str,
    collab_type: &CollabType,
  ) {
    let weak_snapshot_persistence = Arc::downgrade(&self.snapshot_persistence);
    let object_id = object_id.to_string();
    let collab_type = *collab_type;
    let state = self.state.clone();

    if let (Some(db), Some(persistence)) = (
      weak_collab_db.upgrade(),
      weak_snapshot_persistence.upgrade(),
    ) {
      tokio::spawn(async move {
        if let Err(err) =
          Self::try_snapshot(db, persistence, uid, object_id, collab_type, state).await
        {
          tracing::error!("failed to create snapshot: {}", err);
        }
      });
    }
  }

  async fn try_snapshot(
    db: Arc<CollabKVDB>,
    persistence: Arc<dyn SnapshotPersistence>,
    uid: i64,
    object_id: String,
    collab_type: CollabType,
    state: Arc<AtomicU8>,
  ) -> Result<(), PersistenceError> {
    let result: Result<(), PersistenceError> = tokio::task::spawn_blocking(move || {
      let mut collab = Collab::new(uid, object_id.clone(), "1", vec![], false);
      db.read_txn()
        .load_doc_with_txn(uid, &object_id, &mut collab.transact_mut())?;

      // Generate the snapshot
      let txn = collab.transact();
      let encoded_v1 = txn.encode_state_as_update_v1(&StateVector::default());
      persistence.create_snapshot(uid, &object_id, &collab_type, encoded_v1)?;
      Ok(())
    })
    .await
    .unwrap();

    let next_state = if result.is_ok() {
      SnapshotState::Idle
    } else {
      SnapshotState::Fail
    };
    state.store(next_state as u8, Ordering::Release);
    result
  }
}
