use crate::preclude::{lib0Any, MapRefExtension, YrsValue};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::ops::{Deref, DerefMut};
use yrs::{Map, MapRef, ReadTxn, TransactionMut};

pub trait Lib0AnyMapExtension {
  fn value(&self) -> &HashMap<String, lib0Any>;

  fn mut_value(&mut self) -> &mut HashMap<String, lib0Any>;

  fn insert_str_value<K: AsRef<str>>(&mut self, key: K, s: String) {
    let _ = self.mut_value().insert(
      key.as_ref().to_string(),
      lib0Any::String(s.into_boxed_str()),
    );
  }

  fn get_str_value<K: AsRef<str>>(&self, key: K) -> Option<String> {
    let value = self.value().get(key.as_ref())?;
    if let lib0Any::String(s) = value {
      Some(s.to_string())
    } else {
      None
    }
  }

  fn insert_i64_value<K: AsRef<str>>(&mut self, key: K, value: i64) {
    let _ = self
      .mut_value()
      .insert(key.as_ref().to_string(), lib0Any::BigInt(value));
  }

  fn get_i64_value<K: AsRef<str>>(&self, key: K) -> Option<i64> {
    let value = self.value().get(key.as_ref())?;
    if let lib0Any::BigInt(num) = value {
      Some(*num)
    } else {
      None
    }
  }

  fn insert_f64_value<K: AsRef<str>>(&mut self, key: K, value: f64) {
    let _ = self
      .mut_value()
      .insert(key.as_ref().to_string(), lib0Any::Number(value));
  }

  fn get_f64_value<K: AsRef<str>>(&self, key: K) -> Option<f64> {
    let value = self.value().get(key.as_ref())?;
    if let lib0Any::Number(num) = value {
      Some(*num)
    } else {
      None
    }
  }

  fn insert_bool_value<K: AsRef<str>>(&mut self, key: K, value: bool) {
    let _ = self
      .mut_value()
      .insert(key.as_ref().to_string(), lib0Any::Bool(value));
  }

  fn get_bool_value<K: AsRef<str>>(&self, key: K) -> Option<bool> {
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

impl AsRef<AnyMap> for AnyMap {
  fn as_ref(&self) -> &AnyMap {
    self
  }
}

impl AnyMap {
  pub fn new() -> Self {
    Self::default()
  }
}

impl Lib0AnyMapExtension for AnyMap {
  fn value(&self) -> &HashMap<String, lib0Any> {
    &self.0
  }

  fn mut_value(&mut self) -> &mut HashMap<String, lib0Any> {
    &mut self.0
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

  pub fn fill_map_ref(self, txn: &mut TransactionMut, map_ref: MapRef) {
    self.0.into_iter().for_each(|(k, v)| {
      map_ref.insert_with_txn(txn, &k, v);
    })
  }
}

impl<T: ReadTxn> From<(&'_ T, &MapRef)> for AnyMap {
  fn from(params: (&'_ T, &MapRef)) -> Self {
    let mut this = AnyMap::default();
    params.1.iter(params.0).for_each(|(k, v)| {
      if let YrsValue::Any(any) = v {
        this.insert(k.to_string(), any);
      }
    });
    this
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

  /// Insert the lib0Any into the map.
  /// Sometimes you need a integer or a float into the map, you should use [insert_i64_value] or
  /// [insert_f64_value]. Because the integer value will be treated as a float value when calling
  /// this method.
  pub fn insert_any<K: AsRef<str>>(mut self, key: K, value: impl Into<lib0Any>) -> Self {
    let key = key.as_ref();
    self.inner.insert(key.to_string(), value.into());
    self
  }

  pub fn insert_str_value<K: AsRef<str>, S: ToString>(mut self, key: K, s: S) -> Self {
    self.inner.insert_str_value(key, s.to_string());
    self
  }

  /// Insert the i64 into the map.
  pub fn insert_i64_value<K: AsRef<str>>(mut self, key: K, value: i64) -> Self {
    self.inner.insert_i64_value(key, value);
    self
  }

  /// Insert the f64 into the map.
  pub fn insert_f64_value<K: AsRef<str>>(mut self, key: K, value: f64) -> Self {
    self.inner.insert_f64_value(key, value);
    self
  }

  pub fn build(self) -> AnyMap {
    self.inner
  }
}

pub struct AnyMapUpdate<'a, 'b> {
  map_ref: &'a MapRef,
  txn: &'a mut TransactionMut<'b>,
}

impl<'a, 'b> AnyMapUpdate<'a, 'b> {
  pub fn new(txn: &'a mut TransactionMut<'b>, map_ref: &'a MapRef) -> Self {
    Self { txn, map_ref }
  }

  pub fn insert<K: AsRef<str>>(&mut self, key: K, value: impl Into<lib0Any>) {
    let key = key.as_ref();
    self.map_ref.insert_with_txn(self.txn, key, value.into());
  }
}
