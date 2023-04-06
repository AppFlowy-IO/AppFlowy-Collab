use collab::core::any_map::{AnyMap, AnyMapBuilder, AnyMapExtension, AnyMapUpdate};
use collab::preclude::{Map, MapRef, MapRefExtension, ReadTxn, TransactionMut, YrsValue};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::ops::{Deref, DerefMut};

/// Store lists of cells
/// The key is the id of the [Field]
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Cells(HashMap<String, Cell>);

impl Cells {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn into_inner(self) -> HashMap<String, Cell> {
    self.0
  }

  pub fn fill_map_ref(self, txn: &mut TransactionMut, map_ref: &MapRef) {
    self.into_inner().into_iter().for_each(|(k, v)| {
      let cell_map_ref = map_ref.get_or_insert_map_with_txn(txn, &k);
      v.fill_map_ref(txn, cell_map_ref);
    });
  }
}

impl<T: ReadTxn> From<(&'_ T, &MapRef)> for Cells {
  fn from(params: (&'_ T, &MapRef)) -> Self {
    let mut this = Self::new();
    params.1.iter(params.0).for_each(|(k, v)| {
      if let YrsValue::YMap(map_ref) = v {
        this.insert(k.to_string(), (params.0, &map_ref).into());
      }
    });
    this
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

pub struct CellsUpdate<'a, 'b> {
  map_ref: &'a MapRef,
  txn: &'a mut TransactionMut<'b>,
}

impl<'a, 'b> CellsUpdate<'a, 'b> {
  pub fn new(txn: &'a mut TransactionMut<'b>, map_ref: &'a MapRef) -> Self {
    Self { map_ref, txn }
  }

  pub fn insert(self, key: &str, value: Cell) -> Self {
    let cell_map_ref = self.map_ref.get_or_insert_map_with_txn(self.txn, key);
    value.fill_map_ref(self.txn, cell_map_ref);
    self
  }

  /// Override the existing cell's key/value contained in the [Cell]
  /// It will create the cell if it's not exist
  pub fn update(self, key: &str, value: Cell) -> Self {
    let cell_map_ref = self.map_ref.get_or_insert_map_with_txn(self.txn, key);
    value.fill_map_ref(self.txn, cell_map_ref);
    self
  }
}

pub type Cell = AnyMap;
pub type CellBuilder = AnyMapBuilder;
pub type CellUpdate<'a, 'b> = AnyMapUpdate<'a, 'b>;

pub fn get_field_type_from_cell<T: From<i64>>(cell: &Cell) -> Option<T> {
  cell.get_i64_value("field_type").map(|value| T::from(value))
}

pub fn new_cell_builder(field_type: impl Into<i64>) -> CellBuilder {
  let inner = AnyMapBuilder::new();
  inner.insert_i64_value("field_type", field_type.into())
}
