use crate::preclude::{lib0Any, MapRefExtension, MapRefWrapper, YrsValue};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::ops::{Deref, DerefMut};
use yrs::{Map, MapRef, ReadTxn, TransactionMut};

pub trait Lib0AnyMapExtension {
  fn value(&self) -> &HashMap<String, lib0Any>;

  fn get_str_value<K: AsRef<str>>(&self, key: &K) -> Option<String> {
    let value = self.value().get(key.as_ref())?;
    if let lib0Any::String(s) = value {
      Some(s.to_string())
    } else {
      None
    }
  }

  fn get_i64_value<K: AsRef<str>>(&self, key: &K) -> Option<i64> {
    let value = self.value().get(key.as_ref())?;
    if let lib0Any::BigInt(num) = value {
      Some(*num)
    } else {
      None
    }
  }

  fn get_bool_value<K: AsRef<str>>(&self, key: &K) -> Option<bool> {
    let value = self.value().get(key.as_ref())?;
    if let lib0Any::Bool(value) = value {
      Some(*value)
    } else {
      None
    }
  }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AnyMap(HashMap<String, lib0Any>);

impl Lib0AnyMapExtension for AnyMap {
  fn value(&self) -> &HashMap<String, lib0Any> {
    &self.0
  }
}

impl Hash for AnyMap {
  fn hash<H: Hasher>(&self, state: &mut H) {
    self.0.iter().for_each(|(_, v)| {
      v.to_string().hash(state);
    });
  }
}

impl AnyMap {
  pub fn from_map_ref<T: ReadTxn>(txn: &T, map_ref: MapRef) -> Self {
    let mut this = Self(Default::default());
    map_ref.iter(txn).for_each(|(k, v)| {
      if let YrsValue::Any(any) = v {
        this.insert(k.to_string(), any);
      }
    });
    this
  }

  pub fn fill_map_ref(self, txn: &mut TransactionMut, map_ref: MapRefWrapper) {
    self.0.into_iter().for_each(|(k, v)| {
      map_ref.insert_with_txn(txn, &k, v);
    })
  }
}

impl Deref for AnyMap {
  type Target = HashMap<String, lib0Any>;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl DerefMut for AnyMap {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.0
  }
}

pub struct AnyMapBuilder {
  inner: AnyMap,
}

impl AnyMapBuilder {
  pub fn new() -> Self {
    Self {
      inner: Default::default(),
    }
  }

  pub fn insert<K: AsRef<str>>(mut self, key: &str, value: impl Into<lib0Any>) -> Self {
    self.inner.insert(key.to_string(), value.into());
    self
  }

  pub fn build(self) -> AnyMap {
    self.inner
  }
}
