use bytes::Bytes;
use yrs::{Doc, ReadTxn, Transaction, TransactionMut};

pub trait CollabPlugin: Send + Sync + 'static {
    fn did_init(&self, cid: &str, txn: &mut TransactionMut) {}
    fn did_receive_update(&self, cid: &str, txn: &TransactionMut, update: &[u8]);
}
