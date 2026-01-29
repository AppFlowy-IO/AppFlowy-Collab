use std::collections::HashMap;
use std::ops::{Deref, DerefMut};
use std::str::FromStr;

use crate::preclude::{Any, FillRef, Map, MapRef, ReadTxn, ToJson, TransactionMut, YrsValue};
use anyhow::bail;
use serde::{Deserialize, Serialize};
use serde_repr::*;

use crate::util::AnyExt;
use strum_macros::EnumIter;

/// The [DatabaseLayout] enum is used to represent the layout of the database.
#[derive(Debug, PartialEq, Copy, Eq, Hash, Clone, Serialize_repr, Deserialize_repr, EnumIter)]
#[repr(u8)]
pub enum DatabaseLayout {
  Grid = 0,
  Board = 1,
  Calendar = 2,
  Chart = 3,
  List = 4,
  Gallery = 5,
  Feed = 6,
}

impl DatabaseLayout {
  pub fn is_board(&self) -> bool {
    matches!(self, DatabaseLayout::Board)
  }

  pub fn is_chart(&self) -> bool {
    matches!(self, DatabaseLayout::Chart)
  }

  pub fn is_gallery(&self) -> bool {
    matches!(self, DatabaseLayout::Gallery)
  }

  pub fn is_feed(&self) -> bool {
    matches!(self, DatabaseLayout::Feed)
  }
}

impl AsRef<str> for DatabaseLayout {
  fn as_ref(&self) -> &str {
    match self {
      DatabaseLayout::Grid => "0",
      DatabaseLayout::Board => "1",
      DatabaseLayout::Calendar => "2",
      DatabaseLayout::Chart => "3",
      DatabaseLayout::List => "4",
      DatabaseLayout::Gallery => "5",
      DatabaseLayout::Feed => "6",
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
      "3" => Ok(DatabaseLayout::Chart),
      "4" => Ok(DatabaseLayout::List),
      "5" => Ok(DatabaseLayout::Gallery),
      "6" => Ok(DatabaseLayout::Feed),
      _ => bail!("Invalid layout type"),
    }
  }
}

impl Default for DatabaseLayout {
  fn default() -> Self {
    Self::Grid
  }
}

impl From<i64> for DatabaseLayout {
  fn from(value: i64) -> Self {
    match value {
      0 => DatabaseLayout::Grid,
      1 => DatabaseLayout::Board,
      2 => DatabaseLayout::Calendar,
      3 => DatabaseLayout::Chart,
      4 => DatabaseLayout::List,
      5 => DatabaseLayout::Gallery,
      6 => DatabaseLayout::Feed,
      _ => Self::default(),
    }
  }
}

impl From<DatabaseLayout> for Any {
  fn from(layout: DatabaseLayout) -> Self {
    Any::BigInt(layout as i64)
  }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
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
          this.insert(layout, map_ref.to_json(txn).into_map().unwrap());
        }
      }
    });
    this
  }

  /// Fill a [MapRef] with the data from this [LayoutSettings].
  pub fn fill_map_ref(self, txn: &mut TransactionMut, map_ref: &MapRef) {
    self.0.into_iter().for_each(|(k, v)| {
      let inner_map: MapRef = map_ref.get_or_init(txn, k.as_ref());
      Any::from(v).fill(txn, &inner_map).unwrap()
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

/// Each [LayoutSetting] is a [Map] of [String] to [Any].
/// This is used to store the settings for each layout.
pub type LayoutSetting = HashMap<String, Any>;
