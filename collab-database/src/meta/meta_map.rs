use std::ops::Deref;

use collab::preclude::{MapRef, ReadTxn, TransactionMut};

const DATABASE_INLINE_VIEW: &str = "iid";

pub struct MetaMap {
  container: MapRef,
}

impl MetaMap {
  pub fn new(container: MapRef) -> Self {
    Self { container }
  }

  /// Set the inline view id
  pub fn set_inline_view_id_with_txn(&self, txn: &mut TransactionMut, view_id: &str) {
    self
      .container
      .insert_str_with_txn(txn, DATABASE_INLINE_VIEW, view_id);
  }

  /// Get the inline view id
  pub fn get_inline_view_id_with_txn<T: ReadTxn>(&self, txn: &T) -> Option<String> {
    self.container.get_str_with_txn(txn, DATABASE_INLINE_VIEW)
  }
}

impl Deref for MetaMap {
  type Target = MapRef;

  fn deref(&self) -> &Self::Target {
    &self.container
  }
}
