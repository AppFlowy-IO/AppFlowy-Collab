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
      if let YrsValue::YMap(map_ref) = v {
        this.insert(k.to_string(), Cell::from_map_ref(txn, map_ref));
      }
    });
    this
  }

  pub fn fill_map_ref(self, txn: &mut TransactionMut, map_ref: &MapRefWrapper) {
    self.into_inner().into_iter().for_each(|(k, v)| {
      let cell_map_ref = map_ref.get_or_insert_map_with_txn(txn, &k);
      v.fill_map_ref(txn, cell_map_ref);
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
pub struct Cell(HashMap<String, lib0Any>);

impl Cell {
  pub fn from_map_ref<T: ReadTxn>(txn: &T, map_ref: MapRef) -> Self {
    let mut this = Self(Default::default());

    map_ref.iter(txn).for_each(|(k, v)| {
      if let YrsValue::Any(any) = v {
        this.insert(k.to_string(), any);
      }
    });
    this
  }

  pub fn fill_map_ref(self, txn: &mut TransactionMut, map_ref: MapRefWrapper) {
    self.0.into_iter().for_each(|(k, v)| {
      map_ref.insert_with_txn(txn, &k, v);
    })
  }
}

impl Deref for Cell {
  type Target = HashMap<String, lib0Any>;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}
impl DerefMut for Cell {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.0
  }
}
