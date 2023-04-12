use crate::rows::{Cell, Cells};
use collab::preclude::lib0Any;

#[derive(Debug, Default)]
pub struct CellsBuilder {
  cells: Cells,
}

impl CellsBuilder {
  pub fn new() -> CellsBuilder {
    Self::default()
  }

  pub fn insert_cell<T: Into<Cell>>(mut self, key: &str, value: T) -> Self {
    self.cells.insert(key.to_string(), value.into());
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
