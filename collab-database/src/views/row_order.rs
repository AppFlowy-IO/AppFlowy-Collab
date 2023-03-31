use collab::core::array_wrapper::ArrayRefExtension;
use collab::preclude::{
  lib0Any, Array, ArrayRef, ArrayRefWrapper, ReadTxn, TransactionMut, YrsValue,
};
use serde::{Deserialize, Serialize};

pub struct RowOrderArray {
  array_ref: ArrayRef,
}

impl RowOrderArray {
  pub fn new(array_ref: ArrayRef) -> Self {
    Self { array_ref }
  }

  pub fn extends_with_txn(&self, txn: &mut TransactionMut, others: Vec<RowOrder>) {
    let array_ref = ArrayRefExtension(&self.array_ref);
    for row_order in others {
      array_ref.push_back(txn, row_order);
    }
  }

  pub fn get_row_orders_with_txn<T: ReadTxn>(&self, txn: &T) -> Vec<RowOrder> {
    self
      .array_ref
      .iter(txn)
      .flat_map(|v| row_order_from_value(v, txn))
      .collect::<Vec<RowOrder>>()
  }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RowOrder {
  pub id: String,
  pub created_at: i64,
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

pub fn row_order_from_value<T: ReadTxn>(value: YrsValue, txn: &T) -> Option<RowOrder> {
  if let YrsValue::Any(value) = value {
    Some(RowOrder::from(value))
  } else {
    None
  }
}
