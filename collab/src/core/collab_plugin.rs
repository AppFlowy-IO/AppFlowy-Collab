use std::sync::Arc;
use yrs::TransactionMut;

pub trait CollabPlugin: Send + Sync + 'static {
  fn did_init(&self, _object_id: &str, _txn: &mut TransactionMut) {}
  fn did_receive_update(&self, _object_id: &str, _txn: &TransactionMut, _update: &[u8]) {}
  fn after_transaction(&self, _object_id: &str, _txn: &mut TransactionMut) {}
}

impl<T> CollabPlugin for Box<T>
where
  T: CollabPlugin,
{
  fn did_init(&self, object_id: &str, txn: &mut TransactionMut) {
    (**self).did_init(object_id, txn)
  }
  fn did_receive_update(&self, object_id: &str, txn: &TransactionMut, update: &[u8]) {
    (**self).did_receive_update(object_id, txn, update)
  }
}

impl<T> CollabPlugin for Arc<T>
where
  T: CollabPlugin,
{
  fn did_init(&self, object_id: &str, txn: &mut TransactionMut) {
    (**self).did_init(object_id, txn)
  }
  fn did_receive_update(&self, object_id: &str, txn: &TransactionMut, update: &[u8]) {
    (**self).did_receive_update(object_id, txn, update)
  }
}
