use crate::rows::Row;
use crate::views::{OrderArray, OrderIdentifiable};

use collab::preclude::{
  lib0Any, Array, ArrayRef, ArrayRefWrapper, ReadTxn, TransactionMut, YrsValue,
};
use serde::{Deserialize, Serialize};
use std::ops::{Deref, DerefMut};

pub struct RowOrderArray {
  array_ref: ArrayRef,
}

impl OrderArray for RowOrderArray {
  type Object = RowOrder;

  fn array_ref(&self) -> &ArrayRef {
    &self.array_ref
  }

  fn object_from_value_with_txn<T: ReadTxn>(
    &self,
    value: YrsValue,
    txn: &T,
  ) -> Option<Self::Object> {
    row_order_from_value(value, txn)
  }
}

impl RowOrderArray {
  pub fn new(array_ref: ArrayRef) -> Self {
    Self { array_ref }
  }
}

impl Deref for RowOrderArray {
  type Target = ArrayRef;

  fn deref(&self) -> &Self::Target {
    &self.array_ref
  }
}

impl DerefMut for RowOrderArray {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.array_ref
  }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RowOrder {
  pub id: String,
}

impl OrderIdentifiable for RowOrder {
  fn identify_id(&self) -> &str {
    &self.id
  }
}

impl RowOrder {
  pub fn new(id: String) -> RowOrder {
    Self { id }
  }
}

impl From<lib0Any> for RowOrder {
  fn from(any: lib0Any) -> Self {
    let mut json = String::new();
    any.to_json(&mut json);
    serde_json::from_str(&json).unwrap()
  }
}

impl From<RowOrder> for lib0Any {
  fn from(item: RowOrder) -> Self {
    let json = serde_json::to_string(&item).unwrap();
    lib0Any::from_json(&json).unwrap()
  }
}

impl From<&Row> for RowOrder {
  fn from(row: &Row) -> Self {
    Self { id: row.id.clone() }
  }
}

pub fn row_order_from_value<T: ReadTxn>(value: YrsValue, _txn: &T) -> Option<RowOrder> {
  if let YrsValue::Any(value) = value {
    Some(RowOrder::from(value))
  } else {
    None
  }
}
