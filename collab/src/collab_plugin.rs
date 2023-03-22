use bytes::Bytes;
use yrs::{Doc, ReadTxn, TransactionMut};

pub trait CollabPlugin: Send + Sync + 'static {
    fn did_receive_update(&self, txn: &TransactionMut, update: &[u8]);
}
