use std::ops::Deref;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;

use collab_persistence::doc::YrsDocAction;
use collab_persistence::kv::rocks_kv::RocksCollabDB;
use collab_persistence::kv::sled_lv::{SledCollabDB, SledKVStore};
use collab_persistence::kv::KVStore;
use yrs::{Transaction, TransactionMut};

use crate::core::collab_plugin::CollabPlugin;
use crate::error::CollabError;

#[derive(Clone)]
pub struct RocksDiskPlugin {
  uid: i64,
  did_load: Arc<AtomicBool>,
  initial_update_count: Arc<AtomicU32>,
  db: Arc<RocksCollabDB>,
  can_flush: bool,
}

impl Deref for RocksDiskPlugin {
  type Target = Arc<RocksCollabDB>;

  fn deref(&self) -> &Self::Target {
    &self.db
  }
}

impl RocksDiskPlugin {
  pub fn new(uid: i64, db: Arc<RocksCollabDB>) -> Result<Self, CollabError> {
    let did_load = Arc::new(AtomicBool::new(false));
    let initial_update_count = Arc::new(AtomicU32::new(0));
    Ok(Self {
      db,
      uid,
      did_load,
      initial_update_count,
      can_flush: false,
    })
  }
  pub fn new_with_config(
    uid: i64,
    db: Arc<RocksCollabDB>,
    can_flush: bool,
  ) -> Result<Self, CollabError> {
    let did_load = Arc::new(AtomicBool::new(false));
    let initial_update_count = Arc::new(AtomicU32::new(0));
    Ok(Self {
      db,
      uid,
      did_load,
      initial_update_count,
      can_flush,
    })
  }
}

impl CollabPlugin for RocksDiskPlugin {
  fn init(&self, object_id: &str, txn: &mut TransactionMut) {
    let store = self.db.read_txn();
    if store.is_exist(self.uid, object_id) {
      let update_count = store.load_doc(self.uid, object_id, txn).unwrap();
      self
        .initial_update_count
        .store(update_count, Ordering::SeqCst);
    } else {
      drop(store);
      tracing::trace!("🤲collab => {:?} not exist", object_id);
      self.db.with_write_txn(|store| {
        store.create_new_doc(self.uid, object_id, txn).unwrap();
      });
    }
  }

  fn did_init(&self, object_id: &str, txn: &Transaction) {
    let update_count = self.initial_update_count.load(Ordering::SeqCst);
    if update_count > 0 && self.can_flush {
      self.db.with_write_txn(|store| {
        if let Err(e) = store.flush_doc(self.uid, object_id, txn) {
          tracing::error!("Failed to flush doc: {}, error: {:?}", object_id, e);
        } else {
          tracing::trace!("Flush doc: {}", object_id);
        }
      });
    }
    self.did_load.store(true, Ordering::SeqCst);
  }

  fn did_receive_update(&self, object_id: &str, _txn: &TransactionMut, update: &[u8]) {
    if self.did_load.load(Ordering::SeqCst) {
      self.db.with_write_txn(|store| {
        store.push_update(self.uid, object_id, update).unwrap();
      });
    }
  }
}
