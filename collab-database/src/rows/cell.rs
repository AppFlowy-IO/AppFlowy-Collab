use anyhow::{anyhow, bail};
use collab::preclude::{lib0Any, Map, MapRef, MapRefWrapper, ReadTxn, TransactionMut, YrsValue};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::ops::{Deref, DerefMut};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Cells(HashMap<String, Cell>);

impl Cells {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn into_inner(self) -> HashMap<String, Cell> {
    self.0
  }

  pub fn from_map_ref<T: ReadTxn>(txn: &T, map_ref: MapRef) -> Self {
    let mut this = Self::new();
    map_ref.iter(txn).for_each(|(k, v)| {
      if let Ok(value) = Cell::try_from(v) {
        this.insert(k.to_string(), value);
      }
    });
    this
  }

  pub fn fill_in_map_ref(self, txn: &mut TransactionMut, map_ref: &MapRefWrapper) {
    self.into_inner().into_iter().for_each(|(k, v)| {
      map_ref.insert_with_txn(txn, &k, v);
    });
  }
}

impl Deref for Cells {
  type Target = HashMap<String, Cell>;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl DerefMut for Cells {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.0
  }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Cell {
  #[serde(rename = "data")]
  pub type_cell_data: String,
}

impl From<Cell> for lib0Any {
  fn from(value: Cell) -> Self {
    lib0Any::String(value.type_cell_data.into_boxed_str())
  }
}

impl From<Box<str>> for Cell {
  fn from(any: Box<str>) -> Self {
    Self {
      type_cell_data: any.to_string(),
    }
  }
}

impl TryFrom<YrsValue> for Cell {
  type Error = anyhow::Error;

  fn try_from(value: YrsValue) -> Result<Self, Self::Error> {
    if let YrsValue::Any(lib0Any::String(s)) = value {
      Ok(Self::from(s))
    } else {
      bail!("Invalid cell type")
    }
  }
}
