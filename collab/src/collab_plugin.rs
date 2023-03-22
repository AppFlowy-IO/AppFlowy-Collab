use bytes::Bytes;
use yrs::{Doc, ReadTxn};

pub trait CollabPlugin: Send + Sync + 'static {
    fn did_receive_sv(&self, doc: &Doc, sv: &[u8]) {}
}
