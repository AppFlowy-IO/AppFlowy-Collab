use std::sync::Arc;
use yrs::TransactionMut;

pub trait CollabPlugin: Send + Sync + 'static {
  fn did_init(&self, _cid: &str, _txn: &mut TransactionMut) {}
  fn did_receive_update(&self, _cid: &str, _txn: &TransactionMut, _update: &[u8]) {}
  fn after_transaction(&self, _cid: &str, _txn: &mut TransactionMut) {}
}

impl<T> CollabPlugin for Box<T>
where
  T: CollabPlugin,
{
  fn did_init(&self, cid: &str, txn: &mut TransactionMut) {
    (**self).did_init(cid, txn)
  }
  fn did_receive_update(&self, cid: &str, txn: &TransactionMut, update: &[u8]) {
    (**self).did_receive_update(cid, txn, update)
  }
}

impl<T> CollabPlugin for Arc<T>
where
  T: CollabPlugin,
{
  fn did_init(&self, cid: &str, txn: &mut TransactionMut) {
    (**self).did_init(cid, txn)
  }
  fn did_receive_update(&self, cid: &str, txn: &TransactionMut, update: &[u8]) {
    (**self).did_receive_update(cid, txn, update)
  }
}
