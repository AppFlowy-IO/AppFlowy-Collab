use crate::fields::{
  field_from_map_ref, field_from_value, field_id_from_value, Field, FieldBuilder, FieldUpdate,
};
use crate::views::FieldOrder;
use collab::preclude::{Map, MapRefExtension, MapRefWrapper, ReadTxn, TransactionMut};

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
          .set_type_options(field.type_options);
      })
      .done();
  }

  pub fn get_all_fields(&self) -> Vec<Field> {
    let txn = self.container.transact();
    self.get_all_fields_with_txn(&txn)
  }

  pub fn get_field(&self, field_id: &str) -> Option<Field> {
    let txn = self.container.transact();
    self.get_field_with_txn(&txn, field_id)
  }

  pub fn get_field_with_txn<T: ReadTxn>(&self, txn: &T, field_id: &str) -> Option<Field> {
    let map_ref = self.container.get_map_with_txn(txn, field_id)?;
    field_from_map_ref(&map_ref.into_inner(), txn)
  }

  pub fn get_all_fields_with_txn<T: ReadTxn>(&self, txn: &T) -> Vec<Field> {
    self
      .container
      .iter(txn)
      .flat_map(|(_k, v)| field_from_value(v, txn))
      .collect::<Vec<_>>()
  }

  pub fn get_all_field_orders<T: ReadTxn>(&self, txn: &T) -> Vec<FieldOrder> {
    self
      .container
      .iter(txn)
      .flat_map(|(_k, v)| field_id_from_value(v, txn))
      .map(FieldOrder::new)
      .collect::<Vec<_>>()
  }

  pub fn update_field<F>(&self, field_id: &str, f: F)
  where
    F: FnOnce(FieldUpdate),
  {
    self.container.with_transact_mut(|txn| {
      let map_ref = self.container.get_or_insert_map_with_txn(txn, field_id);
      let update = FieldUpdate::new(field_id, txn, &map_ref);
      f(update);
    })
  }

  pub fn delete_field_with_txn(&self, txn: &mut TransactionMut, field_id: &str) {
    self.container.delete_with_txn(txn, field_id);
  }
}
