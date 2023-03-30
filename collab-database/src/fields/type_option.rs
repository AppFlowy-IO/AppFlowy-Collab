use collab::preclude::{Map, MapRef, MapRefWrapper, ReadTxn, TransactionMut};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::ops::{Deref, DerefMut};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TypeOptions(HashMap<String, String>);

impl TypeOptions {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn into_inner(self) -> HashMap<String, String> {
    self.0
  }

  pub fn from_map_ref<T: ReadTxn>(txn: &T, map_ref: MapRef) -> Self {
    let mut this = Self::new();
    map_ref.iter(txn).for_each(|(k, v)| {
      this.insert(k.to_string(), v.to_string(txn));
    });
    this
  }

  pub fn fill_map_ref(self, txn: &mut TransactionMut, map_ref: &MapRefWrapper) {
    self.into_inner().into_iter().for_each(|(k, v)| {
      map_ref.insert_with_txn(txn, &k, v);
    });
  }
}

impl Deref for TypeOptions {
  type Target = HashMap<String, String>;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl DerefMut for TypeOptions {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.0
  }
}
