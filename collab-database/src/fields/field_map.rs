use collab::preclude::{Map, MapExt, MapRef, ReadTxn, Subscription, TransactionMut};

use crate::database::timestamp;
use crate::fields::{
  Field, FieldBuilder, FieldChangeSender, FieldUpdate, field_from_map_ref, field_from_value,
  field_id_from_value, primary_field_id_from_value, subscribe_field_change,
};
use crate::views::FieldOrder;

/// A map of fields
pub struct FieldMap {
  container: MapRef,
  #[allow(dead_code)]
  subscription: Option<Subscription>,
}

impl FieldMap {
  pub fn new(mut container: MapRef, field_change_tx: Option<FieldChangeSender>) -> Self {
    let subscription = field_change_tx.map(|tx| subscribe_field_change(&mut container, tx));
    Self {
      container,
      subscription,
    }
  }

  /// Insert a field into the map with a transaction
  pub fn insert_field(&self, txn: &mut TransactionMut, field: Field) {
    let map_ref: MapRef = self.container.get_or_init(txn, field.id.as_str());
    FieldBuilder::new(&field.id, txn, map_ref)
      .update(|update| {
        update
          .set_name(field.name)
          .set_icon(field.icon)
          .set_created_at(timestamp())
          .set_last_modified(timestamp())
          .set_primary(field.is_primary)
          .set_field_type(field.field_type)
          .set_type_options(field.type_options);
      })
      .done();
  }

  /// Returns the primary field if it exists
  pub fn get_primary_field<T: ReadTxn>(&self, txn: &T) -> Option<Field> {
    for (_, v) in self.container.iter(txn) {
      if let Some(field_id) = primary_field_id_from_value(v, txn) {
        return self.get_field(txn, &field_id);
      }
    }

    None
  }

  /// Get all fields with a transaction
  pub fn get_all_fields<T: ReadTxn>(&self, txn: &T) -> Vec<Field> {
    self.get_fields_with_txn(txn, None)
  }

  /// Return a field by field id with a transaction
  pub fn get_field<T: ReadTxn>(&self, txn: &T, field_id: &str) -> Option<Field> {
    let map_ref: MapRef = self.container.get_with_txn(txn, field_id)?;
    field_from_map_ref(&map_ref, txn)
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
        .flat_map(|(_k, v)| field_from_value(v, txn))
        .collect::<Vec<_>>(),
      Some(field_ids) => self
        .container
        .iter(txn)
        .flat_map(|(_k, v)| field_from_value(v, txn))
        .filter(|field| field_ids.contains(&field.id))
        .collect::<Vec<_>>(),
    }
  }

  /// Returns all field ids
  pub fn number_of_fields<T: ReadTxn>(&self, txn: &T) -> Vec<String> {
    self
      .container
      .iter(txn)
      .flat_map(|(_k, v)| field_id_from_value(v, txn))
      .collect::<Vec<String>>()
  }

  /// Get all field orders with a transaction
  pub fn get_all_field_orders<T: ReadTxn>(&self, txn: &T) -> Vec<FieldOrder> {
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
  pub fn update_field<F>(&self, txn: &mut TransactionMut, field_id: &str, f: F)
  where
    F: FnOnce(FieldUpdate),
  {
    let map_ref: MapRef = self.container.get_or_init(txn, field_id);
    let mut update = FieldUpdate::new(field_id, txn, &map_ref);
    update = update.set_last_modified(timestamp());
    f(update);
  }

  /// Delete a field with a transaction
  pub fn delete_field(&self, txn: &mut TransactionMut, field_id: &str) {
    self.container.remove(txn, field_id);
  }
}
