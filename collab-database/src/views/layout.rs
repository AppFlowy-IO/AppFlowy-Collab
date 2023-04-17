use anyhow::bail;
use collab::core::any_map::{AnyMap, AnyMapBuilder};
use collab::preclude::{lib0Any, Map, MapRef, MapRefExtension, ReadTxn, TransactionMut, YrsValue};
use serde::{Deserialize, Serialize};
use serde_repr::*;
use std::collections::HashMap;
use std::ops::{Deref, DerefMut};
use std::str::FromStr;

/// The [DatabaseLayout] enum is used to represent the layout of the database.
#[derive(Debug, PartialEq, Eq, Hash, Clone, Serialize_repr, Deserialize_repr)]
#[repr(u8)]
pub enum DatabaseLayout {
  Grid = 0,
  Board = 1,
  Calendar = 2,
}

impl AsRef<str> for DatabaseLayout {
  fn as_ref(&self) -> &str {
    match self {
      DatabaseLayout::Grid => "0",
      DatabaseLayout::Board => "1",
      DatabaseLayout::Calendar => "2",
    }
  }
}

impl FromStr for DatabaseLayout {
  type Err = anyhow::Error;

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    match s {
      "0" => Ok(DatabaseLayout::Grid),
      "1" => Ok(DatabaseLayout::Board),
      "2" => Ok(DatabaseLayout::Calendar),
      _ => bail!("Invalid layout type"),
    }
  }
}

impl Default for DatabaseLayout {
  fn default() -> Self {
    Self::Grid
  }
}

impl TryFrom<i64> for DatabaseLayout {
  type Error = anyhow::Error;

  fn try_from(value: i64) -> std::result::Result<Self, Self::Error> {
    match value {
      0 => Ok(DatabaseLayout::Grid),
      1 => Ok(DatabaseLayout::Board),
      2 => Ok(DatabaseLayout::Calendar),
      _ => bail!("Unknown layout type {}", value),
    }
  }
}

impl From<DatabaseLayout> for lib0Any {
  fn from(layout: DatabaseLayout) -> Self {
    lib0Any::BigInt(layout as i64)
  }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LayoutSettings(HashMap<DatabaseLayout, LayoutSetting>);

impl LayoutSettings {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn into_inner(self) -> HashMap<DatabaseLayout, LayoutSetting> {
    self.0
  }

  /// Create a new [LayoutSettings] from a [MapRef].
  pub fn from_map_ref<T: ReadTxn>(txn: &T, map_ref: MapRef) -> Self {
    let mut this = Self::new();
    map_ref.iter(txn).for_each(|(k, v)| {
      if let Ok(layout) = DatabaseLayout::from_str(k) {
        if let YrsValue::YMap(map_ref) = v {
          this.insert(layout, LayoutSetting::from_map_ref(txn, &map_ref));
        }
      }
    });
    this
  }

  /// Fill a [MapRef] with the data from this [LayoutSettings].
  pub fn fill_map_ref(self, txn: &mut TransactionMut, map_ref: &MapRef) {
    self.0.into_iter().for_each(|(k, v)| {
      let inner_map = map_ref.get_or_insert_map_with_txn(txn, k.as_ref());
      v.fill_map_ref(txn, &inner_map);
    });
  }
}

impl Deref for LayoutSettings {
  type Target = HashMap<DatabaseLayout, LayoutSetting>;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl DerefMut for LayoutSettings {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.0
  }
}

/// Each [LayoutSetting] is a [Map] of [String] to [lib0Any].
/// This is used to store the settings for each layout.
pub type LayoutSetting = AnyMap;
pub type LayoutSettingBuilder = AnyMapBuilder;
