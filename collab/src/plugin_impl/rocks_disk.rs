use std::ops::Deref;
use std::sync::atomic::Ordering::SeqCst;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;

use collab_persistence::doc::YrsDocAction;
use collab_persistence::kv::rocks_kv::RocksCollabDB;
use collab_persistence::snapshot::{CollabSnapshot, SnapshotAction};
use yrs::{Transaction, TransactionMut};

use crate::core::collab_plugin::CollabPlugin;
use crate::error::CollabError;

#[derive(Clone)]
pub struct RocksDiskPlugin {
  uid: i64,
  config: Config,
  db: Arc<RocksCollabDB>,
  did_load: Arc<AtomicBool>,
  update_count: Arc<AtomicU32>,
}

impl Deref for RocksDiskPlugin {
  type Target = Arc<RocksCollabDB>;

  fn deref(&self) -> &Self::Target {
    &self.db
  }
}

impl RocksDiskPlugin {
  pub fn new(uid: i64, db: Arc<RocksCollabDB>) -> Result<Self, CollabError> {
    Self::new_with_config(uid, db, Config::default())
  }

  pub fn new_with_config(
    uid: i64,
    db: Arc<RocksCollabDB>,
    config: Config,
  ) -> Result<Self, CollabError> {
    let update_count = Arc::new(AtomicU32::new(0));
    let did_load = Arc::new(AtomicBool::new(false));
    Ok(Self {
      db,
      uid,
      did_load,
      update_count,
      config,
    })
  }

  pub fn get_snapshots(&self, object_id: &str) -> Vec<CollabSnapshot> {
    let transaction = self.db.read_txn();
    transaction.get_snapshots(self.uid, object_id)
  }

  pub fn create_snapshot(&self, txn: &mut TransactionMut, object_id: &str) {
    if let Err(e) = self.db.with_write_txn(|store| {
      store.push_snapshot(self.uid, object_id, "".to_string(), txn)?;
      if self.config.remove_updates_after_snapshot {
        store.delete_all_updates(self.uid, object_id)?;
      }
      Ok(())
    }) {
      tracing::error!("🔴Generate snapshot failed: {}", e);
    }
  }

  fn increase_count(&self) -> u32 {
    self.update_count.fetch_add(1, SeqCst)
  }
}

impl CollabPlugin for RocksDiskPlugin {
  fn init(&self, object_id: &str, txn: &mut TransactionMut) {
    let r_db_txn = self.db.read_txn();
    if r_db_txn.is_exist(self.uid, object_id) {
      let _ = r_db_txn.load_doc(self.uid, object_id, txn).unwrap();
      drop(r_db_txn);

      if self.config.flush_doc {
        let _ = self.db.with_write_txn(|w_db_txn| {
          w_db_txn.flush_doc(self.uid, object_id, txn)?;
          Ok(())
        });
      }
    } else {
      // Drop the read txn before write txn
      let result = self.db.with_write_txn(|w_db_txn| {
        w_db_txn.create_new_doc(self.uid, object_id, txn)?;
        Ok(())
      });

      if let Err(e) = result {
        tracing::warn!("🤲collab => create doc for {:?} failed: {}", object_id, e)
      }
    }
  }

  fn did_init(&self, _object_id: &str, _txn: &Transaction) {
    self.did_load.store(true, Ordering::SeqCst);
  }

  fn did_receive_update(&self, object_id: &str, _txn: &TransactionMut, update: &[u8]) {
    // Only push update if the doc is loaded
    if !self.did_load.load(Ordering::SeqCst) {
      return;
    }
    let result = self.db.with_write_txn(|w_db_txn| {
      w_db_txn.push_update(self.uid, object_id, update)?;
      Ok(())
    });

    if let Err(e) = result {
      tracing::error!("🔴Failed to push update: {:?}", e);
    }
  }

  fn after_transaction(&self, object_id: &str, txn: &mut TransactionMut) {
    // Only push update if the doc is loaded
    if !self.did_load.load(Ordering::SeqCst) {
      return;
    }
    let count = self.increase_count();
    if count != 0 && count % self.config.snapshot_per_update == 0 {
      self.create_snapshot(txn, object_id);
    }
  }
}

#[derive(Clone)]
pub struct Config {
  /// Generate a snapshot every N updates
  /// Default is 100. The value must be greater than 0.
  snapshot_per_update: u32,

  /// Remove updates after snapshot. Default is true.
  /// The snapshot contains all the updates before it. So it's safe to remove them.
  /// But if you want to keep the updates, you can set this to false.
  remove_updates_after_snapshot: bool,

  /// Flush doc after init. Default is false.
  flush_doc: bool,
}

impl Config {
  pub fn new() -> Self {
    let config = Self::default();
    config
  }

  pub fn snapshot_per_update(mut self, snapshot_per_update: u32) -> Self {
    debug_assert!(snapshot_per_update > 0);
    self.snapshot_per_update = snapshot_per_update;
    self
  }

  pub fn remove_updates_after_snapshot(mut self, remove_updates_after_snapshot: bool) -> Self {
    self.remove_updates_after_snapshot = remove_updates_after_snapshot;
    self
  }

  pub fn flush_doc(mut self, flush_doc: bool) -> Self {
    self.flush_doc = flush_doc;
    self
  }
}

impl Default for Config {
  fn default() -> Self {
    Self {
      snapshot_per_update: 100,
      remove_updates_after_snapshot: false,
      flush_doc: false,
    }
  }
}
