pub mod number_type_option;
pub mod select_type_option;
pub mod time_type_option;
pub mod url_type_option;

use std::collections::HashMap;
use std::ops::{Deref, DerefMut};

use crate::rows::Cell;
use crate::template::entity::CELL_DATA;
use collab::preclude::{Any, FillRef, Map, MapRef, ReadTxn, ToJson, TransactionMut};
use collab::util::{AnyExt, AnyMapExt};
use serde::{Deserialize, Serialize};

/// It's used to store lists of field's type option data
/// The key is the [FieldType] string representation
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct TypeOptions(HashMap<String, TypeOptionData>);

impl TypeOptions {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn into_inner(self) -> HashMap<String, TypeOptionData> {
    self.0
  }

  /// Returns a new instance of [TypeOptions] from a [MapRef]
  pub fn from_map_ref<T: ReadTxn>(txn: &T, map_ref: MapRef) -> Self {
    let mut this = Self::new();
    map_ref.iter(txn).for_each(|(k, v)| {
      if let Some(type_option_data) = v.to_json(txn).into_map() {
        this.insert(k.to_string(), type_option_data);
      }
    });
    this
  }

  /// Fill the [MapRef] with the [TypeOptions] data
  pub fn fill_map_ref(self, txn: &mut TransactionMut, map_ref: &MapRef) {
    self.into_inner().into_iter().for_each(|(k, v)| {
      let update = TypeOptionsUpdate::new(txn, map_ref);
      update.insert(&k, v);
    });
  }
}

impl Deref for TypeOptions {
  type Target = HashMap<String, TypeOptionData>;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl DerefMut for TypeOptions {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.0
  }
}

pub struct TypeOptionsUpdate<'a, 'b> {
  map_ref: &'a MapRef,
  txn: &'a mut TransactionMut<'b>,
}

impl<'a, 'b> TypeOptionsUpdate<'a, 'b> {
  pub fn new(txn: &'a mut TransactionMut<'b>, map_ref: &'a MapRef) -> Self {
    Self { map_ref, txn }
  }

  /// Insert a new cell's key/value into the [TypeOptionData]
  pub fn insert<T: Into<TypeOptionData>>(self, key: &str, value: T) -> Self {
    let value = value.into();
    let type_option_map: MapRef = self.map_ref.get_or_init(self.txn, key);
    Any::from(value).fill(self.txn, &type_option_map).unwrap();
    self
  }

  /// Override the existing cell's key/value contained in the [TypeOptionData]
  /// It will create the type option if it's not exist
  pub fn update<T: Into<TypeOptionData>>(self, key: &str, value: T) -> Self {
    let value = value.into();
    let type_option_map: MapRef = self.map_ref.get_or_init(self.txn, key);
    Any::from(value).fill(self.txn, &type_option_map).unwrap();
    self
  }

  /// Remove the cell's key/value from the [TypeOptionData]
  pub fn remove(self, key: &str) -> Self {
    self.map_ref.remove(self.txn, key);
    self
  }
}

pub type TypeOptionData = HashMap<String, Any>;
pub type TypeOptionDataBuilder = HashMap<String, Any>;
pub type TypeOptionUpdate = MapRef;

pub trait StringifyTypeOption {
  fn stringify_cell(&self, cell: &Cell) -> String {
    match cell.get_as::<String>(CELL_DATA) {
      None => "".to_string(),
      Some(s) => Self::stringify_text(self, &s),
    }
  }
  fn stringify_text(&self, text: &str) -> String;
}
