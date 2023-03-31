use collab::core::array_wrapper::ArrayRefExtension;
use collab::preclude::{
  lib0Any, Array, ArrayRef, ArrayRefWrapper, ReadTxn, TransactionMut, YrsValue,
};
use serde::{Deserialize, Serialize};

pub struct FieldOrderArray {
  array_ref: ArrayRef,
}

impl FieldOrderArray {
  pub fn new(array_ref: ArrayRef) -> Self {
    Self { array_ref }
  }

  pub fn extends_with_txn(&self, txn: &mut TransactionMut, others: Vec<FieldOrder>) {
    let array_ref = ArrayRefExtension(&self.array_ref);
    for row_order in others {
      array_ref.push_back(txn, row_order);
    }
  }

  pub fn get_field_orders_with_txn<T: ReadTxn>(&self, txn: &T) -> Vec<FieldOrder> {
    self
      .array_ref
      .iter(txn)
      .flat_map(|v| field_order_from_value(v, txn))
      .collect::<Vec<FieldOrder>>()
  }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FieldOrder {
  pub id: String,
}

impl From<lib0Any> for FieldOrder {
  fn from(any: lib0Any) -> Self {
    let mut json = String::new();
    any.to_json(&mut json);
    serde_json::from_str(&json).unwrap()
  }
}

impl From<FieldOrder> for lib0Any {
  fn from(item: FieldOrder) -> Self {
    let json = serde_json::to_string(&item).unwrap();
    lib0Any::from_json(&json).unwrap()
  }
}

pub fn field_order_from_value<T: ReadTxn>(value: YrsValue, txn: &T) -> Option<FieldOrder> {
  if let YrsValue::Any(value) = value {
    Some(FieldOrder::from(value))
  } else {
    None
  }
}
