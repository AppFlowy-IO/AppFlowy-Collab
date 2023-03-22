use crate::collab_plugin::CollabPlugin;
use bytes::Bytes;
use yrs::{Doc, ReadTxn};

pub struct CollabDiskPlugin {}

impl CollabPlugin for CollabDiskPlugin {
    fn did_receive_sv(&self, doc: &Doc, sv: &[u8]) {
        //
    }
    // let doc_state = txn.encode_state_as_update_v1(&state);
}
