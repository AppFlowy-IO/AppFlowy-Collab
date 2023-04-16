use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use collab_persistence::doc::YrsDocDB;
use collab_persistence::CollabKV;
use yrs::TransactionMut;

use crate::core::collab_plugin::CollabPlugin;
use crate::error::CollabError;

#[derive(Clone)]
pub struct CollabDiskPlugin {
  uid: i64,
  did_load: Arc<AtomicBool>,
  db: Arc<CollabKV>,
}
impl CollabDiskPlugin {
  pub fn new(uid: i64, db: Arc<CollabKV>) -> Result<Self, CollabError> {
    let did_load = Arc::new(AtomicBool::new(false));
    Ok(Self { db, uid, did_load })
  }

  pub fn doc(&self) -> YrsDocDB {
    self.db.doc(self.uid)
  }
}

impl CollabPlugin for CollabDiskPlugin {
  fn did_init(&self, object_id: &str, txn: &mut TransactionMut) {
    let doc = self.doc();
    if doc.is_exist(object_id) {
      doc.load_doc(object_id, txn).unwrap();
    } else {
      self.doc().create_new_doc(object_id, txn).unwrap();
    }
    self.did_load.store(true, Ordering::SeqCst);
  }

  fn did_receive_update(&self, object_id: &str, _txn: &TransactionMut, update: &[u8]) {
    if self.did_load.load(Ordering::SeqCst) {
      self.doc().push_update(object_id, update).unwrap();
    }
  }
}
