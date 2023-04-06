use crate::rows::{Cell, Cells};
use collab::preclude::lib0Any;
use std::collections::HashMap;

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
