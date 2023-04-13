use collab::core::any_map::{AnyMap, AnyMapBuilder, AnyMapUpdate};
use collab::preclude::{Map, MapRef, MapRefExtension, ReadTxn, TransactionMut, YrsValue};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::ops::{Deref, DerefMut};

pub struct FieldTypeOptionKey(i64);

impl ToString for FieldTypeOptionKey {
  fn to_string(&self) -> String {
    self.0.to_string()
  }
}

/// It's used to store lists of field's type option data
/// The key is the [FieldType] string representation
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TypeOptions(HashMap<String, TypeOptionData>);

impl TypeOptions {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn into_inner(self) -> HashMap<String, TypeOptionData> {
    self.0
  }

  pub fn from_map_ref<T: ReadTxn>(txn: &T, map_ref: MapRef) -> Self {
    let mut this = Self::new();
    map_ref.iter(txn).for_each(|(k, v)| {
      if let YrsValue::YMap(map_ref) = v {
        this.insert(k.to_string(), TypeOptionData::from_map_ref(txn, &map_ref));
      }
    });
    this
  }

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

  pub fn insert<T: Into<TypeOptionData>>(self, key: &str, value: T) -> Self {
    let value = value.into();
    let type_option_map = self.map_ref.get_or_insert_map_with_txn(self.txn, key);
    value.fill_map_ref(self.txn, &type_option_map);
    self
  }

  /// Override the existing cell's key/value contained in the [TypeOptionData]
  /// It will create the type option if it's not exist
  pub fn update<T: Into<TypeOptionData>>(self, key: &str, value: T) -> Self {
    let value = value.into();
    let type_option_map = self.map_ref.get_or_insert_map_with_txn(self.txn, key);
    value.fill_map_ref(self.txn, &type_option_map);
    self
  }

  pub fn remove(self, key: &str) -> Self {
    self.map_ref.remove(self.txn, key);
    self
  }
}

pub type TypeOptionData = AnyMap;
pub type TypeOptionDataBuilder = AnyMapBuilder;
pub type TypeOptionUpdate<'a, 'b> = AnyMapUpdate<'a, 'b>;
