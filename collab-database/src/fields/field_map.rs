use collab::preclude::{Map, MapExt, MapRef, ReadTxn, Subscription, TransactionMut};

use crate::database::timestamp;
use crate::fields::{
  field_from_map_ref, field_from_value, field_id_from_value, primary_field_id_from_value,
  subscribe_field_change, Field, FieldBuilder, FieldChangeSender, FieldUpdate,
};
use crate::views::FieldOrder;

/// A map of fields
pub struct FieldMap {
  container: MapRef,
  #[allow(dead_code)]
  subscription: Subscription,
}

impl FieldMap {
  pub fn new(mut container: MapRef, field_change_tx: FieldChangeSender) -> Self {
    let subscription = subscribe_field_change(&mut container, field_change_tx);
    Self {
      container,
      subscription,
    }
  }

  /// Insert a field into the map with a transaction
  pub fn insert_field_with_txn(&self, txn: &mut TransactionMut, field: Field) {
    let map_ref: MapRef = self.container.get_or_init(txn, field.id.as_str());
    FieldBuilder::new(&field.id, txn, map_ref)
      .update(|update| {
        update
          .set_name(field.name)
          .set_created_at(timestamp())
          .set_last_modified(timestamp())
          .set_primary(field.is_primary)
          .set_field_type(field.field_type)
          .set_type_options(field.type_options);
      })
      .done();
  }

  /// Returns the primary field if it exists
  pub fn get_primary_field(&self) -> Option<Field> {
    let txn = self.container.transact();
    for (_, v) in self.container.iter(&txn) {
      if let Some(field_id) = primary_field_id_from_value(v, &txn) {
        return self.get_field_with_txn(&txn, &field_id);
      }
    }

    None
  }

  /// Get all fields
  pub fn get_all_fields(&self) -> Vec<Field> {
    let txn = self.container.transact();
    self.get_all_fields_with_txn(&txn)
  }

  /// Get all fields with a transaction
  pub fn get_all_fields_with_txn<T: ReadTxn>(&self, txn: &T) -> Vec<Field> {
    self.get_fields_with_txn(txn, None)
  }

  /// Return a field by field id
  pub fn get_field(&self, field_id: &str) -> Option<Field> {
    let txn = self.container.transact();
    self.get_field_with_txn(&txn, field_id)
  }

  /// Return a field by field id with a transaction
  pub fn get_field_with_txn<T: ReadTxn>(&self, txn: &T, field_id: &str) -> Option<Field> {
    let map_ref: MapRef = self.container.get_with_txn(txn, field_id)?;
    field_from_map_ref(&map_ref.into_inner(), txn)
  }

  /// Get fields by field ids
  /// If field_ids is None, return all fields
  /// If field_ids is Some, return fields that match the field ids
  pub fn get_fields(&self, field_ids: Option<Vec<String>>) -> Vec<Field> {
    let txn = self.container.transact();
    self.get_fields_with_txn(&txn, field_ids)
  }
  /// Get fields by field ids
  /// If field_ids is None, return all fields
  /// If field_ids is Some, return fields that match the field ids
  pub fn get_fields_with_txn<T: ReadTxn>(
    &self,
    txn: &T,
    field_ids: Option<Vec<String>>,
  ) -> Vec<Field> {
    match field_ids {
      None => self
        .container
        .iter(txn)
        .flat_map(|(_k, v)| field_from_value(&v, txn))
        .collect::<Vec<_>>(),
      Some(field_ids) => self
        .container
        .iter(txn)
        .flat_map(|(_k, v)| field_from_value(&v, txn))
        .filter(|field| field_ids.contains(&field.id))
        .collect::<Vec<_>>(),
    }
  }

  /// Get all field orders
  /// This is used to get the order of fields in the view
  pub fn get_all_field_orders(&self) -> Vec<FieldOrder> {
    let txn = self.container.transact();
    self.get_all_field_orders_with_txn(&txn)
  }

  /// Returns all field ids
  pub fn number_of_fields(&self) -> Vec<String> {
    let txn = self.container.transact();
    self
      .container
      .iter(&txn)
      .flat_map(|(_k, v)| field_id_from_value(v, &txn))
      .collect::<Vec<String>>()
  }

  /// Get all field orders with a transaction
  /// This is used to get the order of fields in the view
  pub fn get_all_field_orders_with_txn<T: ReadTxn>(&self, txn: &T) -> Vec<FieldOrder> {
    self
      .container
      .iter(txn)
      .flat_map(|(_k, v)| field_id_from_value(v, txn))
      .map(FieldOrder::new)
      .collect::<Vec<_>>()
  }

  /// Update a field
  /// This is used to update the field. This changes will be reflected
  /// all the views that use this field
  pub fn update_field<F>(&self, field_id: &str, f: F)
  where
    F: FnOnce(FieldUpdate),
  {
    self.container.with_transact_mut(|txn| {
      let map_ref: MapRef = self.container.get_or_init(txn, field_id);
      let mut update = FieldUpdate::new(field_id, txn, &map_ref);
      update = update.set_last_modified(timestamp());
      f(update);
    })
  }

  /// Delete a field with a transaction
  pub fn delete_field_with_txn(&self, txn: &mut TransactionMut, field_id: &str) {
    self.container.delete_with_txn(txn, field_id);
  }
}
