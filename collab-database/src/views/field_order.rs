use collab::preclude::{lib0Any, ArrayRefWrapper};
use serde::{Deserialize, Serialize};

pub struct FieldOrderArray {
  container: ArrayRefWrapper,
}

impl FieldOrderArray {
  pub fn new(container: ArrayRefWrapper) -> Self {
    Self { container }
  }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FieldOrder {
  pub id: String,
  pub created_at: i64,
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
