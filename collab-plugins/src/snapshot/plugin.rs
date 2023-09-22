use std::panic;
use std::panic::AssertUnwindSafe;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Weak};

use collab::preclude::{Collab, CollabPlugin};
use collab_define::CollabObject;
use collab_persistence::doc::YrsDocAction;
use collab_persistence::kv::rocks_kv::RocksCollabDB;
use collab_persistence::snapshot::{CollabSnapshot, SnapshotAction};
use collab_persistence::PersistenceError;
use parking_lot::RwLock;
use similar::{ChangeTag, TextDiff};
use yrs::updates::decoder::Decode;
use yrs::{ReadTxn, StateVector, TransactionMut, Update};

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

pub trait SnapshotPersistence: Send + Sync {
  fn get_snapshots(&self, uid: i64, object_id: &str) -> Vec<CollabSnapshot>;

  fn create_snapshot(
    &self,
    uid: i64,
    object_id: &str,
    title: String,
    snapshot_data: Vec<u8>,
  ) -> Result<(), PersistenceError>;
}

pub struct CollabSnapshotPlugin {
  uid: i64,
  object: CollabObject,
  collab_db: Weak<RocksCollabDB>,
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
    collab_db: Weak<RocksCollabDB>,
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
              let snapshot_data = txn.encode_state_as_update_v1(&StateVector::default());
              match snapshot_persistence.create_snapshot(
                uid,
                &object.object_id,
                object.collab_type.to_string(),
                snapshot_data,
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

impl SnapshotPersistence for Arc<RocksCollabDB> {
  fn get_snapshots(&self, uid: i64, object_id: &str) -> Vec<CollabSnapshot> {
    self.read_txn().get_snapshots(uid, object_id)
  }

  fn create_snapshot(
    &self,
    uid: i64,
    object_id: &str,
    _title: String,
    snapshot_data: Vec<u8>,
  ) -> Result<(), PersistenceError> {
    self.with_write_txn(|txn| {
      txn.create_snapshot_with_data(uid, object_id, snapshot_data)?;
      Ok(())
    })
  }
}

pub fn calculate_snapshot_diff(
  uid: i64,
  object_id: &str,
  old_snapshot: &[u8],
  new_snapshot: &[u8],
) -> Result<String, anyhow::Error> {
  if old_snapshot.is_empty() {
    return Ok("".to_string());
  }

  if new_snapshot.is_empty() {
    return Err(anyhow::anyhow!(
      "The new {} snapshot data is empty",
      object_id
    ));
  }

  let old = try_decode_snapshot(uid, object_id, old_snapshot)?;
  let new = try_decode_snapshot(uid, object_id, new_snapshot)?;

  let mut display_str = String::new();
  let diff = TextDiff::from_lines(&old, &new);
  for change in diff.iter_all_changes() {
    let sign = match change.tag() {
      ChangeTag::Delete => "-",
      ChangeTag::Insert => "+",
      ChangeTag::Equal => " ",
    };
    display_str.push_str(&format!("{}{}", sign, change));
  }
  Ok(display_str)
}

pub fn try_decode_snapshot(
  uid: i64,
  object_id: &str,
  data: &[u8],
) -> Result<String, PersistenceError> {
  let mut decoded_str = String::new();
  match {
    let mut wrapper = AssertUnwindSafe(&mut decoded_str);
    panic::catch_unwind(move || {
      let collab = Collab::new(uid, object_id, "1", vec![]);
      if let Ok(update) = Update::decode_v1(data) {
        let mut txn = collab.origin_transact_mut();
        txn.apply_update(update);
        drop(txn);
      }
      **wrapper = collab.to_plain_text();
    })
  } {
    Ok(_) => Ok(decoded_str),
    Err(e) => Err(PersistenceError::InvalidData(format!("{:?}", e))),
  }
}
