use std::ops::{Deref, DerefMut};

use collab::preclude::{Any, Array, ArrayRef, ReadTxn, TransactionMut, YrsValue};
use serde::{Deserialize, Serialize};

use crate::fields::Field;
use crate::views::{OrderArray, OrderIdentifiable};

/// Keep track of the order of fields in a database view
pub struct FieldOrderArray {
  array_ref: ArrayRef,
}

impl OrderArray for FieldOrderArray {
  type Object = FieldOrder;

  /// Return a reference to the underlying array
  fn array_ref(&self) -> &ArrayRef {
    &self.array_ref
  }

  /// Create a new [SFieldOrder] instance from the given value
  fn object_from_value<T: ReadTxn>(&self, value: YrsValue, txn: &T) -> Option<Self::Object> {
    field_order_from_value(value, txn)
  }
}

impl FieldOrderArray {
  pub fn new(array_ref: ArrayRef) -> Self {
    Self { array_ref }
  }

  pub fn extends_with_txn(&self, txn: &mut TransactionMut, others: Vec<FieldOrder>) {
    for row_order in others {
      self.array_ref.push_back(txn, row_order);
    }
  }

  pub fn get_field_orders_with_txn<T: ReadTxn>(&self, txn: &T) -> Vec<FieldOrder> {
    self
      .array_ref
      .iter(txn)
      .flat_map(|v| field_order_from_value(v, txn))
      .collect::<Vec<FieldOrder>>()
  }

  pub fn remove_with_txn(&self, txn: &mut TransactionMut, field_id: &str) -> Option<()> {
    let pos =
      self
        .array_ref
        .iter(txn)
        .position(|value| match field_order_from_value(value, txn) {
          None => false,
          Some(field_order) => field_order.id == field_id,
        })?;
    self.array_ref.remove(txn, pos as u32);
    None
  }
}

impl Deref for FieldOrderArray {
  type Target = ArrayRef;

  fn deref(&self) -> &Self::Target {
    &self.array_ref
  }
}

impl DerefMut for FieldOrderArray {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.array_ref
  }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct FieldOrder {
  pub id: String,
}

impl OrderIdentifiable for FieldOrder {
  fn identify_id(&self) -> String {
    self.id.clone()
  }
}

impl FieldOrder {
  pub fn new(id: String) -> FieldOrder {
    Self { id }
  }
}

impl From<&Field> for FieldOrder {
  fn from(field: &Field) -> Self {
    Self {
      id: field.id.clone(),
    }
  }
}

impl From<Any> for FieldOrder {
  fn from(any: Any) -> Self {
    let mut json = String::new();
    any.to_json(&mut json);
    serde_json::from_str(&json).unwrap()
  }
}

impl From<FieldOrder> for Any {
  fn from(item: FieldOrder) -> Self {
    let json = serde_json::to_string(&item).unwrap();
    Any::from_json(&json).unwrap()
  }
}

pub fn field_order_from_value<T: ReadTxn>(value: YrsValue, _txn: &T) -> Option<FieldOrder> {
  if let YrsValue::Any(value) = value {
    Some(FieldOrder::from(value))
  } else {
    None
  }
}
