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

#[derive(Debug, Default)]
pub struct CellsBuilder {
  cells: Cells,
}

impl CellsBuilder {
  pub fn new() -> CellsBuilder {
    Self::default()
  }

  pub fn insert_cell<T: Into<lib0Any>>(mut self, key: &str, value: HashMap<String, T>) -> Self {
    let mut cell = Cell::new();
    value.into_iter().for_each(|(k, v)| {
      cell.insert(k, v.into());
    });
    self.cells.insert(key.to_string(), cell);
    self
  }

  pub fn insert_text_cell<T: Into<TextCell>>(mut self, key: &str, text_cell: T) -> Self {
    let text_cell = text_cell.into();
    self.cells.insert(key.to_string(), text_cell.into());
    self
  }

  pub fn build(self) -> Cells {
    self.cells
  }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Cell(HashMap<String, lib0Any>);

impl Cell {
  pub fn new() -> Self {
    Self::default()
  }
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

pub struct TextCell(String);

impl From<TextCell> for Cell {
  fn from(text_cell: TextCell) -> Self {
    let mut cell = Self::new();
    cell.insert(
      "data".to_string(),
      lib0Any::String(text_cell.0.into_boxed_str()),
    );
    cell
  }
}

impl From<String> for TextCell {
  fn from(s: String) -> Self {
    Self(s)
  }
}

impl From<&str> for TextCell {
  fn from(s: &str) -> Self {
    Self(s.to_string())
  }
}
