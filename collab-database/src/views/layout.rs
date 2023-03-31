use anyhow::bail;
use collab::preclude::{lib0Any, MapRef, MapRefExtension, MapRefWrapper, ReadTxn, TransactionMut};
use serde::{Deserialize, Serialize};
use serde_repr::*;
use std::collections::HashMap;

#[derive(Debug, PartialEq, Eq, Hash, Clone, Serialize_repr, Deserialize_repr)]
#[repr(u8)]
pub enum Layout {
  Grid = 0,
  Board = 1,
  Calendar = 2,
}

impl AsRef<str> for Layout {
  fn as_ref(&self) -> &str {
    match self {
      Layout::Grid => "Grid",
      Layout::Board => "Board",
      Layout::Calendar => "Calendar",
    }
  }
}

impl Default for Layout {
  fn default() -> Self {
    Self::Grid
  }
}

impl TryFrom<i64> for Layout {
  type Error = anyhow::Error;

  fn try_from(value: i64) -> std::result::Result<Self, Self::Error> {
    match value {
      0 => Ok(Layout::Grid),
      1 => Ok(Layout::Board),
      2 => Ok(Layout::Calendar),
      _ => bail!("Unknown layout type {}", value),
    }
  }
}

impl From<Layout> for lib0Any {
  fn from(layout: Layout) -> Self {
    lib0Any::BigInt(layout as i64)
  }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LayoutSettings(HashMap<Layout, LayoutSetting>);

impl LayoutSettings {
  pub fn from_map_ref<T: ReadTxn>(_txn: &T, map_ref: MapRef) -> Self {
    let _map_ref = MapRefExtension(&map_ref);
    todo!()
  }

  pub fn fill_map_ref(self, txn: &mut TransactionMut, map_ref: &MapRefWrapper) {
    self.0.into_iter().for_each(|(k, v)| {
      let inner_map = map_ref.get_or_insert_map_with_txn(txn, k.as_ref());
      v.fill_map_ref(txn, &inner_map);
    });
  }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LayoutSetting(HashMap<String, String>);

impl LayoutSetting {
  pub fn fill_map_ref(self, txn: &mut TransactionMut, map_ref: &MapRefWrapper) {
    self.0.into_iter().for_each(|(k, v)| {
      map_ref.insert_with_txn(txn, &k, v);
    });
  }
}
