use std::ops::{Deref, DerefMut};

use collab::preclude::{Any, ArrayRef, ReadTxn, YrsValue};
use collab::util::deserialize_i32_from_numeric;
use serde::{Deserialize, Serialize};

use crate::rows::{Row, RowId};
use crate::views::{OrderArray, OrderIdentifiable};

pub struct RowOrderArray {
  array_ref: ArrayRef,
}

impl OrderArray for RowOrderArray {
  type Object = RowOrder;

  fn array_ref(&self) -> &ArrayRef {
    &self.array_ref
  }

  fn object_from_value<T: ReadTxn>(&self, value: YrsValue, txn: &T) -> Option<Self::Object> {
    row_order_from_value(&value, txn)
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

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct RowOrder {
  pub id: RowId,

  #[serde(deserialize_with = "deserialize_i32_from_numeric")]
  pub height: i32,
}

impl OrderIdentifiable for RowOrder {
  fn identify_id(&self) -> String {
    self.id.to_string()
  }
}

impl RowOrder {
  pub fn new(id: RowId, height: i32) -> RowOrder {
    Self { id, height }
  }
}

impl From<&Any> for RowOrder {
  fn from(any: &Any) -> Self {
    let mut json = String::new();
    any.to_json(&mut json);
    serde_json::from_str(&json).unwrap()
  }
}

impl From<RowOrder> for Any {
  fn from(item: RowOrder) -> Self {
    let json = serde_json::to_string(&item).unwrap();
    Any::from_json(&json).unwrap()
  }
}

impl From<&Row> for RowOrder {
  fn from(row: &Row) -> Self {
    Self {
      id: row.id.clone(),
      height: row.height,
    }
  }
}

impl From<&RowOrder> for RowOrder {
  fn from(row: &RowOrder) -> Self {
    row.clone()
  }
}
pub fn row_order_from_value<T: ReadTxn>(value: &YrsValue, _txn: &T) -> Option<RowOrder> {
  if let YrsValue::Any(value) = value {
    Some(RowOrder::from(value))
  } else {
    None
  }
}
