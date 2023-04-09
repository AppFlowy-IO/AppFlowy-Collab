use crate::rows::{BlockId, Row, RowId};
use crate::views::{OrderArray, OrderIdentifiable};
use collab::preclude::{lib0Any, ArrayRef, ReadTxn, YrsValue};
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
  pub id: RowId,
  pub block_id: BlockId,
  pub height: i32,
}

impl OrderIdentifiable for RowOrder {
  fn identify_id(&self) -> String {
    self.id.to_string()
  }
}

impl RowOrder {
  pub fn new(id: RowId, block_id: BlockId, height: i32) -> RowOrder {
    Self {
      id,
      block_id,
      height,
    }
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
    Self {
      id: row.id,
      block_id: row.block_id,
      height: row.height,
    }
  }
}

pub fn row_order_from_value<T: ReadTxn>(value: YrsValue, _txn: &T) -> Option<RowOrder> {
  if let YrsValue::Any(value) = value {
    Some(RowOrder::from(value))
  } else {
    None
  }
}
