use std::panic;
use std::panic::AssertUnwindSafe;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use collab::core::collab::MutexCollab;
use collab::preclude::{Collab, CollabPlugin};
use collab_persistence::kv::rocks_kv::RocksCollabDB;
use collab_persistence::snapshot::{CollabSnapshot, SnapshotAction};
use collab_persistence::PersistenceError;
use parking_lot::{Mutex, RwLock};
use similar::{ChangeTag, TextDiff};
use yrs::updates::decoder::Decode;
use yrs::{ReadTxn, Snapshot, TransactionMut, Update};

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

pub trait SnapshotDB: Send + Sync {
  fn get_snapshots(&self, uid: i64, object_id: &str) -> Vec<CollabSnapshot>;

  fn create_snapshot(
    &self,
    uid: i64,
    object_id: &str,
    snapshot: Snapshot,
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
  current_snapshot: Arc<Mutex<Option<Snapshot>>>,
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
      current_snapshot: Default::default(),
    }
  }

  /// Return the snapshots for the given object id
  pub fn get_snapshots(&self, object_id: &str) -> Vec<CollabSnapshot> {
    self.db.get_snapshots(self.uid, object_id)
  }
}

impl CollabPlugin for CollabSnapshotPlugin {
  fn after_transaction(&self, object_id: &str, txn: &mut TransactionMut) {
    let mut current_snapshot = self.current_snapshot.lock();
    if current_snapshot.is_none() {
      *current_snapshot = Some(txn.snapshot())
    }

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
        let weak_local_collab = Arc::downgrade(&self.local_collab);
        let weak_state = Arc::downgrade(&self.state);
        let weak_db = Arc::downgrade(&self.db);
        let uid = self.uid;
        let object_id = object_id.to_string();
        let weak_snapshot = Arc::downgrade(&self.current_snapshot);

        // We use a blocking task to generate the snapshot
        tokio::spawn(async move {
          let _ = tokio::task::spawn_blocking(move || {
            if let (Some(state), Some(local_collab), Some(db), Some(weak_snapshot)) = (
              weak_state.upgrade(),
              weak_local_collab.upgrade(),
              weak_db.upgrade(),
              weak_snapshot.upgrade(),
            ) {
              if let Some(snapshot) = weak_snapshot.lock().take() {
                // Create a new snapshot that contains all the document data. If the snapshot
                // generation fails, we set the state to fail, so that the next transaction
                // will try to generate a new snapshot again.
                if let Err(e) = db.create_snapshot(uid, &object_id, snapshot, local_collab) {
                  tracing::error!("{} snapshot generation failed: {}", object_id, e);
                  *state.write() = GenSnapshotState::Fail;
                } else {
                  *state.write() = GenSnapshotState::Idle;
                }
              } else {
                *state.write() = GenSnapshotState::Idle;
              }
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
    snapshot: Snapshot,
    collab: Arc<MutexCollab>,
  ) -> Result<(), PersistenceError> {
    self.with_write_txn(|txn| {
      txn.push_snapshot(uid, object_id, &collab.lock().transact(), snapshot)?;
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
      let collab = Collab::new(uid, object_id, vec![]);
      if let Ok(update) = Update::decode_v1(data) {
        let mut txn = collab.transact_mut();
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
