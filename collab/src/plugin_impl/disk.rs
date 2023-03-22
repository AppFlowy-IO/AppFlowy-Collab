use crate::core::collab_plugin::CollabPlugin;
use crate::error::CollabError;
use bytes::Bytes;
use collab_persistence::doc::YrsDoc;
use collab_persistence::{CollabKV, PersistenceError};
use std::path::Path;
use yrs::{Doc, ReadTxn, Transaction, TransactionMut};

#[derive(Clone)]
pub struct CollabDiskPlugin {
    db: CollabKV,
}
impl CollabDiskPlugin {
    pub fn new(path: impl AsRef<Path>) -> Result<Self, CollabError> {
        let db = CollabKV::open(path)?;
        Ok(Self { db })
    }

    pub fn create(&self, id: String, txn: Transaction) -> Result<(), CollabError> {
        let _ = self.db.doc().insert_or_create_new_doc(&id, &txn)?;
        Ok(())
    }

    pub fn doc(&self) -> YrsDoc {
        self.db.doc()
    }
}

impl CollabPlugin for CollabDiskPlugin {
    fn did_init(&self, cid: &str, txn: &mut TransactionMut) {
        // let cid = cid.to_string();
        let doc = self.db.doc();
        if doc.is_exist(cid) {
            doc.load_doc(cid, txn).unwrap();
        } else {
            self.db.doc().insert_or_create_new_doc(cid, txn).unwrap();
        }
    }

    fn did_receive_update(&self, cid: &str, txn: &TransactionMut, update: &[u8]) {
        self.db.doc().push_update(cid, update).unwrap();
    }
    // let doc_state = txn.encode_state_as_update_v1(&state);
}
