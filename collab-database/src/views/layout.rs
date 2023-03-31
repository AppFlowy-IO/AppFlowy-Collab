use anyhow::bail;
use collab::preclude::{
  lib0Any, Map, MapRef, MapRefExtension, MapRefWrapper, ReadTxn, TransactionMut, YrsValue,
};
use serde::{Deserialize, Serialize};
use serde_repr::*;
use std::collections::HashMap;
use std::ops::{Deref, DerefMut};
use std::str::FromStr;

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

impl FromStr for Layout {
  type Err = anyhow::Error;

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    match s {
      "Grid" => Ok(Layout::Grid),
      "Board" => Ok(Layout::Board),
      "Calendar" => Ok(Layout::Calendar),
      _ => bail!("Invalid layout type"),
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
  pub fn new() -> Self {
    Self::default()
  }

  pub fn into_inner(self) -> HashMap<Layout, LayoutSetting> {
    self.0
  }

  pub fn from_map_ref<T: ReadTxn>(txn: &T, map_ref: MapRef) -> Self {
    let mut this = Self::new();
    map_ref.iter(txn).for_each(|(k, v)| {
      if let Ok(layout) = Layout::from_str(k) {
        if let YrsValue::YMap(map_ref) = v {
          this.insert(layout, LayoutSetting::from_map_ref(txn, map_ref));
        }
      }
    });
    this
  }

  pub fn fill_map_ref(self, txn: &mut TransactionMut, map_ref: &MapRef) {
    let map_ref = MapRefExtension(map_ref);
    self.0.into_iter().for_each(|(k, v)| {
      let inner_map = map_ref.get_or_insert_map_with_txn(txn, k.as_ref());
      v.fill_map_ref(txn, &inner_map);
    });
  }
}

impl Deref for LayoutSettings {
  type Target = HashMap<Layout, LayoutSetting>;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl DerefMut for LayoutSettings {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.0
  }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LayoutSetting(HashMap<String, lib0Any>);

impl LayoutSetting {
  pub fn from_map_ref<T: ReadTxn>(txn: &T, map_ref: MapRef) -> Self {
    let mut this = Self(Default::default());
    map_ref.iter(txn).for_each(|(k, v)| {
      if let YrsValue::Any(any) = v {
        this.insert(k.to_string(), any);
      }
    });
    this
  }

  pub fn fill_map_ref(self, txn: &mut TransactionMut, map_ref: &MapRef) {
    let map_ref_ext = MapRefExtension(map_ref);
    self.0.into_iter().for_each(|(k, v)| {
      map_ref_ext.insert_with_txn(txn, &k, v);
    });
  }
}
impl Deref for LayoutSetting {
  type Target = HashMap<String, lib0Any>;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}
impl DerefMut for LayoutSetting {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.0
  }
}
