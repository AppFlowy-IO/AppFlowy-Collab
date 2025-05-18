use std::collections::HashMap;
use std::ops::Deref;

use collab::preclude::{Any, FillRef, Map, MapRef, TransactionMut};
use collab::util::AnyMapExt;

use crate::database::timestamp;
use crate::rows::{CREATED_AT, LAST_MODIFIED, RowId};
use crate::template::entity::CELL_DATA;

pub type Cells = HashMap<String, Cell>;

pub struct CellsUpdate<'a, 'b> {
  map_ref: &'a MapRef,
  txn: &'a mut TransactionMut<'b>,
}

impl<'a, 'b> CellsUpdate<'a, 'b> {
  pub fn new(txn: &'a mut TransactionMut<'b>, map_ref: &'a MapRef) -> Self {
    Self { map_ref, txn }
  }

  pub fn insert_cell(self, key: &str, cell: Cell) -> Self {
    let cell_map_ref: MapRef = self.map_ref.get_or_init(self.txn, key);
    if cell_map_ref.get(self.txn, CREATED_AT).is_none() {
      cell_map_ref.insert(self.txn, CREATED_AT, Any::BigInt(timestamp()));
    }

    Any::from(cell).fill(self.txn, &cell_map_ref).unwrap();
    cell_map_ref.insert(self.txn, LAST_MODIFIED, Any::BigInt(timestamp()));
    self
  }

  /// Override the existing cell's key/value contained in the [Cell]
  /// It will create the cell if it's not exist
  pub fn insert<T: Into<Cell>>(self, key: &str, value: T) -> Self {
    let cell = value.into();
    self.insert_cell(key, cell)
  }

  pub fn clear(self, key: &str) -> Self {
    let cell_map_ref: MapRef = self.map_ref.get_or_init(self.txn, key);
    cell_map_ref.clear(self.txn);

    self
  }
}

pub type Cell = HashMap<String, Any>;
pub type CellBuilder = HashMap<String, Any>;
pub type CellUpdate = MapRef;

pub const CELL_FIELD_TYPE: &str = "field_type";
pub fn get_field_type_from_cell<T: From<i64>>(cell: &Cell) -> Option<T> {
  let field_type: i64 = cell.get_as(CELL_FIELD_TYPE)?;
  Some(T::from(field_type))
}

/// Create a new [CellBuilder] with the field type.
pub fn new_cell_builder(field_type: impl Into<i64>) -> CellBuilder {
  HashMap::from([(CELL_FIELD_TYPE.into(), Any::BigInt(field_type.into()))])
}

pub struct RowCell {
  pub row_id: RowId,
  /// The cell might be empty if no value is written before
  pub cell: Option<Cell>,
}

impl RowCell {
  pub fn new(row_id: RowId, cell: Option<Cell>) -> Self {
    Self { row_id, cell }
  }

  pub fn text(&self) -> Option<String> {
    self
      .cell
      .as_ref()
      .and_then(|cell| cell.get_as::<String>(CELL_DATA))
  }
}

impl Deref for RowCell {
  type Target = Option<Cell>;

  fn deref(&self) -> &Self::Target {
    &self.cell
  }
}
