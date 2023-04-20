use crate::error::CollabError;
use crate::preclude::CollabPlugin;
use collab_persistence::kv::sled_lv::{SledCollabDB, SledKVStore};
use collab_persistence::snapshot::SnapshotAction;
use std::sync::atomic::AtomicU32;
use std::sync::atomic::Ordering::SeqCst;
use std::sync::Arc;
use yrs::TransactionMut;

#[derive(Clone)]
pub struct CollabSnapshotPlugin {
  pub uid: i64,
  pub db: Arc<SledCollabDB>,
  update_count: Arc<AtomicU32>,
  snapshot_per_txn: u32,
}

impl CollabSnapshotPlugin {
  pub fn new(uid: i64, db: Arc<SledCollabDB>, snapshot_per_txn: u32) -> Result<Self, CollabError> {
    let update_count = Arc::new(AtomicU32::new(0));
    Ok(Self {
      uid,
      db,
      update_count,
      snapshot_per_txn,
    })
  }

  fn increase_count(&self) -> u32 {
    self.update_count.fetch_add(1, SeqCst)
  }
}

impl CollabPlugin for CollabSnapshotPlugin {
  fn after_transaction(&self, object_id: &str, txn: &mut TransactionMut) {
    let count = self.increase_count();
    if count != 0 && count % self.snapshot_per_txn == 0 {
      let snapshot = self.db.kv_store_impl();
      // generate snapshot
      if let Err(err) = snapshot.push_snapshot(self.uid, object_id, "".to_string(), txn) {
        tracing::error!("🔴Generate snapshot failed: {}", err);
      }
    }
  }
}
