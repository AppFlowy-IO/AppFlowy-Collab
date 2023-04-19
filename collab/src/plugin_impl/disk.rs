use lib0::error::Error;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;

use collab_persistence::doc::YrsDocDB;
use collab_persistence::CollabDB;
use yrs::updates::decoder::Decode;
use yrs::{Transaction, TransactionMut, Update};

use crate::core::collab_plugin::CollabPlugin;
use crate::error::CollabError;

#[derive(Clone)]
pub struct CollabDiskPlugin {
  uid: i64,
  did_load: Arc<AtomicBool>,
  initial_update_count: Arc<AtomicU32>,
  db: Arc<CollabDB>,
  can_flush: bool,
}

impl CollabDiskPlugin {
  pub fn new(uid: i64, db: Arc<CollabDB>) -> Result<Self, CollabError> {
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
    db: Arc<CollabDB>,
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

  pub fn doc(&self) -> YrsDocDB {
    self.db.doc(self.uid)
  }
}

impl CollabPlugin for CollabDiskPlugin {
  fn init(&self, object_id: &str, txn: &mut TransactionMut) {
    let doc = self.doc();
    if doc.is_exist(object_id) {
      let update_count = doc.load_doc(object_id, txn).unwrap();
      self
        .initial_update_count
        .store(update_count, Ordering::SeqCst);
    } else {
      tracing::trace!("🤲collab => {:?} not exist", object_id);
      self.doc().create_new_doc(object_id, txn).unwrap();
    }
  }

  fn did_init(&self, object_id: &str, txn: &Transaction) {
    let update_count = self.initial_update_count.load(Ordering::SeqCst);
    if update_count > 0 && self.can_flush {
      if let Err(e) = self.doc().flush_doc(object_id, txn) {
        tracing::error!("Failed to flush doc: {}, error: {:?}", object_id, e);
      } else {
        tracing::trace!("Flush doc: {}", object_id);
      }
    }
    // if update_count > 0 {
    //   if let Err(e) = self.doc().flush_doc(object_id, txn) {
    //     tracing::error!("🔴 Flush doc failed: {}, error: {:?}", object_id, e);
    //   } else {
    //     tracing::trace!("Flush doc: {}", object_id);
    //   }
    // }

    self.did_load.store(true, Ordering::SeqCst);
  }

  fn did_receive_update(&self, object_id: &str, _txn: &TransactionMut, update: &[u8]) {
    if self.did_load.load(Ordering::SeqCst) {
      self.doc().push_update(object_id, update).unwrap();
    }
  }
}
