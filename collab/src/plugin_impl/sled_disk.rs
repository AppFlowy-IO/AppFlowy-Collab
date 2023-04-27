use std::ops::Deref;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;

use collab_persistence::doc::YrsDocAction;
use collab_persistence::kv::sled_lv::SledCollabDB;
use yrs::{Doc, Transaction, TransactionMut};

use crate::core::collab_plugin::CollabPlugin;
use crate::error::CollabError;

#[derive(Clone)]
pub struct SledDiskPlugin {
  uid: i64,
  did_load: Arc<AtomicBool>,
  db: Arc<SledCollabDB>,
}

impl Deref for SledDiskPlugin {
  type Target = Arc<SledCollabDB>;

  fn deref(&self) -> &Self::Target {
    &self.db
  }
}

impl SledDiskPlugin {
  pub fn new(uid: i64, db: Arc<SledCollabDB>) -> Result<Self, CollabError> {
    let did_load = Arc::new(AtomicBool::new(false));
    Ok(Self { db, uid, did_load })
  }
}

impl CollabPlugin for SledDiskPlugin {
  fn init(&self, object_id: &str, txn: &mut TransactionMut) {
    let doc = self.db.read_txn();
    if doc.is_exist(self.uid, object_id) {
      let _ = doc.load_doc(self.uid, object_id, false, txn).unwrap();
    } else {
      tracing::trace!("🤲collab => {:?} not exist", object_id);
      doc.create_new_doc(self.uid, object_id, txn).unwrap();
    }
  }

  fn did_init(&self, doc: &Doc, object_id: &str, txn: &Transaction) {
    self.did_load.store(true, Ordering::SeqCst);
  }

  fn did_receive_update(&self, object_id: &str, _txn: &TransactionMut, update: &[u8]) {
    if self.did_load.load(Ordering::SeqCst) {
      self
        .db
        .read_txn()
        .push_update(self.uid, object_id, update)
        .unwrap();
    }
  }
}
