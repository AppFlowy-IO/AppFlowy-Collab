use crate::fields::{field_from_map_ref, Field, FieldBuilder, FieldUpdate};
use collab::preclude::{MapRefWrapper, ReadTxn, TransactionMut};

pub struct FieldMap {
  container: MapRefWrapper,
}

impl FieldMap {
  pub fn new(container: MapRefWrapper) -> Self {
    Self { container }
  }

  pub fn insert_field(&self, field: Field) {
    self.container.with_transact_mut(|txn| {
      self.insert_field_with_txn(txn, field);
    });
  }

  pub fn insert_field_with_txn(&self, txn: &mut TransactionMut, field: Field) {
    let map_ref = self.container.insert_map_with_txn(txn, &field.id);
    FieldBuilder::new(&field.id, txn, map_ref)
      .update(|update| {
        update
          .set_name(field.name)
          .set_primary(field.is_primary)
          .set_field_type(field.field_type)
          .set_width(field.width)
          .set_visibility(field.visibility)
          .set_type_option(field.type_options);
      })
      .done();
  }

  pub fn get_field_with_txn<T: ReadTxn>(&self, txn: &T, field_id: &str) -> Option<Field> {
    let map_ref = self.container.get_map_with_txn(txn, field_id)?;
    field_from_map_ref(&map_ref.into_inner(), txn)
  }

  pub fn update_field<F>(&self, field_id: &str, f: F) -> Option<Field>
  where
    F: FnOnce(FieldUpdate) -> Option<Field>,
  {
    self.container.with_transact_mut(|txn| {
      let map_ref = self.container.get_map_with_txn(txn, field_id)?;
      let update = FieldUpdate::new(field_id, txn, &map_ref);
      f(update)
    })
  }
}
