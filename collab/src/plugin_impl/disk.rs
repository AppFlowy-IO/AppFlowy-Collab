use crate::core::collab_plugin::CollabPlugin;
use crate::error::CollabError;

use collab_persistence::doc::YrsDoc;
use collab_persistence::CollabKV;

use std::sync::Arc;
use yrs::TransactionMut;

#[derive(Clone)]
pub struct CollabDiskPlugin {
  uid: i64,
  db: Arc<CollabKV>,
}
impl CollabDiskPlugin {
  pub fn new(uid: i64, db: Arc<CollabKV>) -> Result<Self, CollabError> {
    Ok(Self { db, uid })
  }

  pub fn doc(&self) -> YrsDoc {
    self.db.doc(self.uid)
  }
}

impl CollabPlugin for CollabDiskPlugin {
  fn did_init(&self, object_id: &str, txn: &mut TransactionMut) {
    let doc = self.doc();
    if doc.is_exist(object_id) {
      doc.load_doc(object_id, txn).unwrap();
    } else {
      self.doc().insert_or_create_new_doc(object_id, txn).unwrap();
    }
  }

  fn did_receive_update(&self, object_id: &str, _txn: &TransactionMut, update: &[u8]) {
    self.doc().push_update(object_id, update).unwrap();
  }
}
