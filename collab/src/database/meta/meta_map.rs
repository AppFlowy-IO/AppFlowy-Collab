use crate::entity::define::DATABASE_INLINE_VIEW;
use crate::preclude::{Any, Map, MapRef, ReadTxn, TransactionMut};
use std::ops::Deref;
use tracing::error;

pub struct MetaMap {
  container: MapRef,
}

pub const DATABASE_ROW_TEMPLATES: &str = "row_templates";

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
  pub fn set_row_templates_json(&self, txn: &mut TransactionMut, json: &str) {
    self
      .container
      .insert(txn, DATABASE_ROW_TEMPLATES, Any::String(json.into()));
  }

  pub fn get_row_templates_json<T: ReadTxn>(&self, txn: &T) -> Option<String> {
    self
      .container
      .get(txn, DATABASE_ROW_TEMPLATES)?
      .cast::<String>()
      .ok()
  }

}

impl Deref for MetaMap {
  type Target = MapRef;

  fn deref(&self) -> &Self::Target {
    &self.container
  }
}
