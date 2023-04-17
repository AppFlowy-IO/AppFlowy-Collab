use collab::preclude::{Doc, MapRef, MapRefExtension, MapRefWrapper, ReadTxn, TransactionMut};
use std::ops::Deref;
const DATABASE_INLINE_VIEW: &str = "iid";

pub struct MetaMap {
  container: MapRefWrapper,
}

impl MetaMap {
  pub fn new(container: MapRefWrapper) -> Self {
    Self { container }
  }

  pub fn insert_doc(&self, doc: Doc) {
    self.container.insert("1", doc);
  }

  /// Set the inline view id
  pub fn set_inline_view_with_txn(&self, txn: &mut TransactionMut, view_id: &str) {
    self
      .container
      .insert_str_with_txn(txn, DATABASE_INLINE_VIEW, view_id);
  }

  /// Get the inline view id
  pub fn get_inline_view_with_txn<T: ReadTxn>(&self, txn: &T) -> Option<String> {
    self.container.get_str_with_txn(txn, DATABASE_INLINE_VIEW)
  }
}

impl Deref for MetaMap {
  type Target = MapRef;

  fn deref(&self) -> &Self::Target {
    &self.container
  }
}
