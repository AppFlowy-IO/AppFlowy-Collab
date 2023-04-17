use crate::rows::{Cell, Cells};

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
