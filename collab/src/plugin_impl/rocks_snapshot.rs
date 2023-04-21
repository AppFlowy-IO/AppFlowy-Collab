use std::sync::atomic::AtomicU32;
use std::sync::atomic::Ordering::SeqCst;
use std::sync::Arc;

use collab_persistence::kv::rocks_kv::RocksCollabDB;
use collab_persistence::snapshot::SnapshotAction;
use yrs::TransactionMut;

use crate::error::CollabError;
use crate::preclude::CollabPlugin;

#[derive(Clone)]
pub struct RocksSnapshotPlugin {
  pub uid: i64,
  pub db: Arc<RocksCollabDB>,
  update_count: Arc<AtomicU32>,
  snapshot_per_txn: u32,
}

impl RocksSnapshotPlugin {
  pub fn new(uid: i64, db: Arc<RocksCollabDB>, snapshot_per_txn: u32) -> Result<Self, CollabError> {
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

impl CollabPlugin for RocksSnapshotPlugin {
  fn after_transaction(&self, object_id: &str, txn: &mut TransactionMut) {
    let count = self.increase_count();
    if count != 0 && count % self.snapshot_per_txn == 0 {
      match self
        .db
        .with_write_txn(|store| store.push_snapshot(self.uid, object_id, "".to_string(), txn))
      {
        Ok(_) => {},
        Err(e) => tracing::error!("🔴Generate snapshot failed: {}", e),
      }
    }
  }
}
