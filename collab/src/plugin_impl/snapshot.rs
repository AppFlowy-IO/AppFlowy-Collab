use crate::error::CollabError;
use crate::preclude::CollabPlugin;
use collab_persistence::snapshot::YrsSnapshot;
use collab_persistence::CollabKV;
use std::sync::atomic::AtomicU32;
use std::sync::atomic::Ordering::SeqCst;
use std::sync::Arc;
use yrs::TransactionMut;

#[derive(Clone)]
pub struct CollabSnapshotPlugin {
  uid: i64,
  db: Arc<CollabKV>,
  update_count: Arc<AtomicU32>,
  snapshot_per_txn: u32,
}

impl CollabSnapshotPlugin {
  pub fn new(uid: i64, db: Arc<CollabKV>, snapshot_per_txn: u32) -> Result<Self, CollabError> {
    let update_count = Arc::new(AtomicU32::new(0));
    Ok(Self {
      uid,
      db,
      update_count,
      snapshot_per_txn,
    })
  }

  pub fn snapshot(&self) -> YrsSnapshot {
    self.db.snapshot(self.uid)
  }

  fn increase_count(&self) -> u32 {
    self.update_count.fetch_add(1, SeqCst)
  }
}

impl CollabPlugin for CollabSnapshotPlugin {
  fn after_transaction(&self, object_id: &str, txn: &mut TransactionMut) {
    let count = self.increase_count();
    if count != 0 && count % self.snapshot_per_txn == 0 {
      // generate snapshot
      if let Err(err) = self
        .snapshot()
        .push_snapshot(object_id, "".to_string(), txn)
      {
        tracing::error!("Generate snapshot failed: {}", err);
      }
    }
  }
}
