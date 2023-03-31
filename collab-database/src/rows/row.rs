use crate::rows::Cells;
use crate::{impl_any_update, impl_bool_update, impl_i32_update, impl_i64_update, impl_str_update};
use collab::preclude::{
  lib0Any, Map, MapRef, MapRefExtension, MapRefWrapper, ReadTxn, TransactionMut, YrsValue,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Row {
  pub id: String,
  pub cells: Cells,
  pub height: i32,
  pub visibility: bool,
}

pub struct RowBuilder<'a, 'b> {
  id: &'a str,
  map_ref: MapRefWrapper,
  txn: &'a mut TransactionMut<'b>,
}

impl<'a, 'b> RowBuilder<'a, 'b> {
  pub fn new(id: &'a str, txn: &'a mut TransactionMut<'b>, map_ref: MapRefWrapper) -> Self {
    map_ref.insert_with_txn(txn, ROW_ID, id);
    Self { id, map_ref, txn }
  }

  pub fn update<F>(self, f: F) -> Self
  where
    F: FnOnce(RowUpdate),
  {
    let update = RowUpdate::new(self.id, self.txn, &self.map_ref);
    f(update);
    self
  }
  pub fn done(self) {}
}

pub struct RowUpdate<'a, 'b, 'c> {
  id: &'a str,
  map_ref: &'c MapRefWrapper,
  txn: &'a mut TransactionMut<'b>,
}

impl<'a, 'b, 'c> RowUpdate<'a, 'b, 'c> {
  pub fn new(id: &'a str, txn: &'a mut TransactionMut<'b>, map_ref: &'c MapRefWrapper) -> Self {
    Self { id, map_ref, txn }
  }

  impl_bool_update!(set_visibility, set_visibility_if_not_none, ROW_VISIBILITY);
  impl_i32_update!(set_height, set_height_at_if_not_none, ROW_HEIGHT);

  pub fn set_cells(self, cells: Cells) -> Self {
    let cell_map = self.map_ref.get_or_insert_map_with_txn(self.txn, ROW_CELLS);
    cells.fill_in_map_ref(self.txn, &cell_map);
    self
  }

  pub fn done(self) -> Option<Row> {
    row_from_map_ref(self.map_ref, self.txn)
  }
}

const ROW_ID: &str = "id";
const ROW_VISIBILITY: &str = "visibility";
const ROW_HEIGHT: &str = "height";
const ROW_CELLS: &str = "cells";

pub fn row_from_value<T: ReadTxn>(value: YrsValue, txn: &T) -> Option<Row> {
  let map_ref = value.to_ymap()?;
  row_from_map_ref(&map_ref, txn)
}

pub fn row_from_map_ref<T: ReadTxn>(map_ref: &MapRef, txn: &T) -> Option<Row> {
  let map_ref = MapRefExtension(map_ref);
  let id = map_ref.get_str_with_txn(txn, ROW_ID)?;
  let visibility = map_ref
    .get_bool_with_txn(txn, ROW_VISIBILITY)
    .unwrap_or(true);

  let height = map_ref.get_i64_with_txn(txn, ROW_HEIGHT).unwrap_or(60);

  let cells = map_ref
    .get_map_ref_with_txn(txn, ROW_CELLS)
    .map(|map_ref| Cells::from_map_ref(txn, map_ref))
    .unwrap_or_default();

  Some(Row {
    id,
    cells,
    height: height as i32,
    visibility,
  })
}
