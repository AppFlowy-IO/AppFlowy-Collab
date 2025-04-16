use collab::preclude::{Any, Map, MapRef, ReadTxn, TransactionMut};
use collab_entity::define::DATABASE_INLINE_VIEW;
use std::ops::Deref;
use tracing::error;

pub struct MetaMap {
  container: MapRef,
}

impl MetaMap {
  pub fn new(container: MapRef) -> Self {
    Self { container }
  }

  /// Set the inline view id
  pub(crate) fn set_inline_view_id(&self, txn: &mut TransactionMut, view_id: &str) {
    self
      .container
      .insert(txn, DATABASE_INLINE_VIEW, Any::String(view_id.into()));
  }

  /// Get the inline view id
  pub(crate) fn get_inline_view_id<T: ReadTxn>(&self, txn: &T) -> Option<String> {
    let out = self.container.get(txn, DATABASE_INLINE_VIEW);
    if out.is_none() {
      error!("Can't find inline view id");
    }

    match out?.cast::<String>() {
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
