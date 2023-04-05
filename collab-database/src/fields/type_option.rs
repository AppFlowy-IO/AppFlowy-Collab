use collab::core::lib0_any_ext::{AnyMap, AnyMapBuilder, Lib0AnyMapExtension};
use collab::preclude::{
  lib0Any, Map, MapRef, MapRefExtension, MapRefWrapper, ReadTxn, TransactionMut, YrsValue,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::ops::{Deref, DerefMut};

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
        this.insert(k.to_string(), TypeOptionData::from_map_ref(txn, map_ref));
      }
    });
    this
  }

  pub fn fill_map_ref(self, txn: &mut TransactionMut, map_ref: &MapRefWrapper) {
    self.into_inner().into_iter().for_each(|(k, v)| {
      let type_option_map = map_ref.get_or_insert_map_with_txn(txn, &k);
      v.fill_map_ref(txn, type_option_map);
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

pub type TypeOptionData = AnyMap;
pub type TypeOptionDataBuilder = AnyMapBuilder;
