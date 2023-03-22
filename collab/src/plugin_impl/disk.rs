use crate::collab_plugin::CollabPlugin;
use crate::error::CollabError;
use bytes::Bytes;
use collab_persistence::CollabKV;
use std::path::Path;
use yrs::{Doc, ReadTxn, TransactionMut};

pub struct CollabDiskPlugin {
    db: CollabKV,
}
impl CollabDiskPlugin {
    pub fn new(path: impl AsRef<Path>) -> Result<Self, CollabError> {
        let db = CollabKV::open(path)?;
        Ok(Self { db })
    }
}

impl CollabPlugin for CollabDiskPlugin {
    fn did_receive_update(&self, txn: &TransactionMut, update: &[u8]) {
        todo!()
    }
    // let doc_state = txn.encode_state_as_update_v1(&state);
}
