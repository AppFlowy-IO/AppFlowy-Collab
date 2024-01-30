use std::ops::Deref;

use collab::preclude::{MapRef, MapRefExtension, MapRefWrapper, ReadTxn, TransactionMut};

const DATABASE_NAME: &str = "name";
const DATABASE_INLINE_VIEW: &str = "iid";

pub struct MetaMap {
  container: MapRefWrapper,
}

impl MetaMap {
  pub fn new(container: MapRefWrapper) -> Self {
    Self { container }
  }

  /// Set the name of the database
  pub fn set_name_with_txn(&self, txn: &mut TransactionMut, name: &str) {
    self.container.insert_str_with_txn(txn, DATABASE_NAME, name)
  }

  /// Get the name of the database
  pub fn get_name_with_txn<T: ReadTxn>(&self, txn: &T) -> String {
    self
      .container
      .get_str_with_txn(txn, DATABASE_NAME)
      .unwrap_or_default()
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
