use std::ops::Deref;

use collab::preclude::{Map, MapRef, ReadTxn, TransactionMut};

pub const DATABASE_INLINE_VIEW: &str = "iid";

pub struct MetaMap {
  container: MapRef,
}

impl MetaMap {
  pub fn new(container: MapRef) -> Self {
    Self { container }
  }

  /// Set the inline view id
  pub fn set_inline_view_id(&self, txn: &mut TransactionMut, view_id: &str) {
    self.container.insert(txn, DATABASE_INLINE_VIEW, view_id);
  }

  /// Get the inline view id
  pub fn get_inline_view_id<T: ReadTxn>(&self, txn: &T) -> Option<String> {
    self.container.get(txn, DATABASE_INLINE_VIEW)?.cast().ok()
  }
}

impl Deref for MetaMap {
  type Target = MapRef;

  fn deref(&self) -> &Self::Target {
    &self.container
  }
}
