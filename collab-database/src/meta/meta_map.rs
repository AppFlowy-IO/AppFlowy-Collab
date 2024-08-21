use collab::preclude::{Any, Map, MapRef, ReadTxn, TransactionMut};
use std::ops::Deref;
use tracing::error;

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
    self
      .container
      .insert(txn, DATABASE_INLINE_VIEW, Any::String(view_id.into()));
  }

  /// Get the inline view id
  pub fn get_inline_view_id<T: ReadTxn>(&self, txn: &T) -> Option<String> {
    match self
      .container
      .get(txn, DATABASE_INLINE_VIEW)?
      .cast::<String>()
    {
      Ok(id) => Some(id),
      Err(err) => {
        error!("Failed to cast inline view id: {:?}", err);
        None
      },
    }
  }
}

impl Deref for MetaMap {
  type Target = MapRef;

  fn deref(&self) -> &Self::Target {
    &self.container
  }
}
