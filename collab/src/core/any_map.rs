use std::collections::HashMap;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::ops::{Deref, DerefMut};
use std::sync::Arc;

use serde::de::{MapAccess, Visitor};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use yrs::{Any, Array, Map, MapRef, ReadTxn, TransactionMut};

use crate::preclude::{JsonValue, MapRefExtension, YrsValue};

/// A wrapper around `yrs::Map` that provides a more ergonomic API.
pub trait AnyMapExtension {
  fn value(&self) -> &HashMap<String, Any>;

  fn mut_value(&mut self) -> &mut HashMap<String, Any>;

  /// Insert the string value with the given key.
  fn insert_str_value<K: AsRef<str>>(&mut self, key: K, s: String) {
    let _ = self
      .mut_value()
      .insert(key.as_ref().to_string(), Any::String(Arc::<str>::from(s)));
  }

  /// Get the string value with the given key.
  fn get_str_value<K: AsRef<str>>(&self, key: K) -> Option<String> {
    let value = self.value().get(key.as_ref())?;
    if let Any::String(s) = value {
      Some(s.to_string())
    } else {
      None
    }
  }

  /// Insert the i64 value with the given key.
  fn insert_i64_value<K: AsRef<str>>(&mut self, key: K, value: i64) {
    let _ = self
      .mut_value()
      .insert(key.as_ref().to_string(), Any::BigInt(value));
  }

  /// Get the i64 value with the given key.
  fn get_i64_value<K: AsRef<str>>(&self, key: K) -> Option<i64> {
    let value = self.value().get(key.as_ref())?;
    if let Any::BigInt(num) = value {
      Some(*num)
    } else {
      None
    }
  }

  /// Insert the f64 value with the given key.
  fn insert_f64_value<K: AsRef<str>>(&mut self, key: K, value: f64) {
    let _ = self
      .mut_value()
      .insert(key.as_ref().to_string(), Any::Number(value));
  }

  /// Get the f64 value with the given key.
  fn get_f64_value<K: AsRef<str>>(&self, key: K) -> Option<f64> {
    let value = self.value().get(key.as_ref())?;
    if let Any::Number(num) = value {
      Some(*num)
    } else {
      None
    }
  }

  /// Insert the bool value with the given key.
  fn insert_bool_value<K: AsRef<str>>(&mut self, key: K, value: bool) {
    let _ = self
      .mut_value()
      .insert(key.as_ref().to_string(), Any::Bool(value));
  }

  /// Get the bool value with the given key.
  fn get_bool_value<K: AsRef<str>>(&self, key: K) -> Option<bool> {
    let value = self.value().get(key.as_ref())?;
    if let Any::Bool(value) = value {
      Some(*value)
    } else {
      None
    }
  }

  /// Get the maps with the given key.
  fn get_array<K: AsRef<str>, T: From<AnyMap>>(&self, key: K) -> Vec<T> {
    if let Some(Any::Array(array)) = self.value().get(key.as_ref()) {
      return array
        .iter()
        .flat_map(|item| {
          if let Any::Map(map) = item {
            Some(T::from(AnyMap(map.clone())))
          } else {
            None
          }
        })
        .collect::<Vec<_>>();
    }
    vec![]
  }

  /// Try to get the maps with the given key.
  /// It [T] can't be converted from [AnyMap], it will be ignored.
  fn try_get_array<K: AsRef<str>, T: TryFrom<AnyMap>>(&self, key: K) -> Vec<T> {
    if let Some(Any::Array(array)) = self.value().get(key.as_ref()) {
      return array
        .iter()
        .flat_map(|item| {
          if let Any::Map(map) = item {
            T::try_from(AnyMap(map.clone())).ok()
          } else {
            None
          }
        })
        .collect::<Vec<_>>();
    }
    vec![]
  }

  /// Insert the maps with the given key.
  /// It will override the old maps with the same id.
  fn insert_array<K: AsRef<str>, T: Into<AnyMap>>(&mut self, key: K, items: Vec<T>) {
    let key = key.as_ref();
    let array = items_to_lib_0_array(items);
    self.mut_value().insert(key.to_string(), array);
  }

  /// Extends the maps with the given key.
  fn extend_with_array<K: AsRef<str>, T: Into<AnyMap>>(&mut self, key: K, items: Vec<T>) {
    let key = key.as_ref();
    let items = items_to_anys(items);
    if let Some(Any::Array(old_items)) = self.value().get(key) {
      let mut new_items = old_items.to_vec();
      new_items.extend(items);
      self
        .mut_value()
        .insert(key.to_string(), Any::Array(Arc::from(new_items)));
    } else {
      self
        .mut_value()
        .insert(key.to_string(), items_to_lib_0_array(items));
    }
  }

  /// Remove the maps with the given ids.
  /// It requires the element to have an [id] field. Otherwise, it will be ignored.
  fn remove_array_element<K: AsRef<str>>(&mut self, key: K, ids: &[&str]) {
    if let Some(Any::Array(array)) = self.value().get(key.as_ref()) {
      let new_array = array
        .iter()
        .filter(|item| {
          if let Any::Map(map) = item {
            if let Some(Any::String(s)) = map.get("id") {
              return !ids.contains(&(*s).as_ref());
            }
          }
          true
        })
        .cloned()
        .collect::<Vec<Any>>();

      self.mut_value().insert(
        key.as_ref().to_string(),
        Any::Array(Arc::<[Any]>::from(new_array)),
      );
    }
  }
}

#[inline]
fn items_to_lib_0_array<T: Into<AnyMap>>(items: Vec<T>) -> Any {
  let items = items_to_anys(items);
  Any::Array(Arc::from(items))
}

#[inline]
fn items_to_anys<T: Into<AnyMap>>(items: Vec<T>) -> Vec<Any> {
  items
    .into_iter()
    .map(|item| {
      let any_map: AnyMap = item.into();
      any_map.into() // Any::Map
    })
    .collect::<Vec<_>>()
}

pub struct MutAnyMap<'a>(&'a mut HashMap<String, Any>);

impl<'a> AnyMapExtension for MutAnyMap<'a> {
  fn value(&self) -> &HashMap<String, Any> {
    self.0
  }

  fn mut_value(&mut self) -> &mut HashMap<String, Any> {
    self.0
  }
}

/// A map that can store any type of value.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AnyMap(Arc<HashMap<String, Any>>);

impl AsRef<AnyMap> for AnyMap {
  fn as_ref(&self) -> &AnyMap {
    self
  }
}

impl AnyMap {
  pub fn new() -> Self {
    Self::default()
  }
  pub fn into_inner(self) -> Arc<HashMap<String, Any>> {
    self.0
  }

  pub fn extend(&mut self, other: AnyMap) {
    let mut_map = Arc::make_mut(&mut self.0);
    other.0.iter().for_each(|(k, v)| {
      mut_map.insert(k.to_string(), v.clone());
    });
  }
}

impl AnyMapExtension for AnyMap {
  fn value(&self) -> &HashMap<String, Any> {
    &self.0
  }

  fn mut_value(&mut self) -> &mut HashMap<String, Any> {
    Arc::make_mut(&mut self.0)
  }
}

// FixMe: https://github.com/georust/geo/issues/391
#[allow(clippy::derived_hash_with_manual_eq)]
impl Hash for AnyMap {
  fn hash<H: Hasher>(&self, state: &mut H) {
    self.0.iter().for_each(|(_, v)| {
      v.to_string().hash(state);
    });
  }
}

impl AnyMap {
  pub fn from_map_ref<T: ReadTxn>(txn: &T, map_ref: &MapRef) -> Self {
    (txn, map_ref).into()
  }

  pub fn from_value<T: ReadTxn>(txn: &T, value: &YrsValue) -> Option<Self> {
    if let YrsValue::YMap(map_ref) = value {
      Some(Self::from_map_ref(txn, map_ref))
    } else {
      None
    }
  }

  /// Insert the content of [AnyMap] into the input map_ref
  pub fn fill_map_ref(self, txn: &mut TransactionMut, map_ref: &MapRef) {
    self.0.iter().for_each(|(k, v)| match v {
      Any::Array(array) => {
        map_ref.create_array_with_txn(txn, k, array.to_vec());
      },
      Any::BigInt(num) => {
        map_ref.insert_i64_with_txn(txn, k, *num);
      },
      Any::Number(num) => {
        map_ref.insert_f64_with_txn(txn, k, *num);
      },
      _ => {
        map_ref.insert_with_txn(txn, k, v.clone());
      },
    })
  }
}

impl From<AnyMap> for Any {
  fn from(map: AnyMap) -> Self {
    Any::Map(map.0)
  }
}

impl From<Any> for AnyMap {
  fn from(value: Any) -> Self {
    if let Any::Map(map) = value {
      Self(map)
    } else {
      Self::default()
    }
  }
}

impl From<&Any> for AnyMap {
  fn from(value: &Any) -> Self {
    if let Any::Map(map) = value {
      Self(map.clone())
    } else {
      Self::default()
    }
  }
}

impl<T: ReadTxn> From<(&'_ T, &MapRef)> for AnyMap {
  fn from(params: (&'_ T, &MapRef)) -> Self {
    let (txn, map_ref) = params;
    let mut this = AnyMap::default();
    map_ref.iter(txn).for_each(|(k, v)| match v {
      YrsValue::Any(any) => {
        this.insert(k.to_string(), any);
      },
      YrsValue::YMap(map) => {
        let map = map
          .iter(txn)
          .flat_map(|(inner_k, inner_v)| {
            if let YrsValue::Any(any) = inner_v {
              Some((inner_k.to_string(), any))
            } else {
              None
            }
          })
          .collect::<HashMap<String, Any>>();
        this.insert(k.to_string(), Any::Map(Arc::new(map)));
      },
      YrsValue::YArray(array) => {
        let array = array
          .iter(txn)
          .flat_map(|v| {
            if let YrsValue::Any(any) = v {
              Some(any)
            } else {
              None
            }
          })
          .collect::<Vec<Any>>();
        this.insert(k.to_string(), Any::Array(Arc::from(array)));
      },
      _ => {
        debug_assert!(false, "Unsupported");
      },
    });
    this
  }
}

impl Serialize for AnyMap {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: Serializer,
  {
    let map = Arc::as_ref(&self.0);
    let mut serialize_map = serializer.serialize_map(Some(map.len()))?;
    for (k, v) in map {
      match v {
        Any::Number(num) => {
          serialize_map.serialize_entry(k, num)?;
        },
        Any::BigInt(num) => {
          serialize_map.serialize_entry(k, num)?;
        },
        _ => {
          serialize_map.serialize_entry(k, v)?;
        },
      }
    }
    serialize_map.end()
  }
}

struct AnyMapVisitor;

impl<'de> Visitor<'de> for AnyMapVisitor {
  type Value = AnyMap;

  fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
    formatter.write_str("a map with string keys and mixed value types")
  }

  fn visit_map<V>(self, mut map: V) -> Result<AnyMap, V::Error>
  where
    V: MapAccess<'de>,
  {
    let mut any_map = HashMap::new();
    let mut error = None;

    // Custom Serialization/Deserialization for `Any`:
    // The default serde implementation for `Any` converts integer values to floating-point values.
    // For instance, an integer like 1 would be serialized as 1.0 (a float), which is not desirable in our use case.
    // To prevent this, we implement custom serialization and deserialization for `Any` to ensure that
    // integers remain as integers and floats as floats, preserving their original types during the process.
    while let Some((key, value)) = map.next_entry::<String, JsonValue>()? {
      let any_value = match &value {
        JsonValue::Number(num) => {
          if let Some(n) = num.as_i64() {
            Any::BigInt(n)
          } else if let Some(n) = num.as_f64() {
            Any::Number(n)
          } else {
            error = Some(serde::de::Error::custom("number is too big"));
            break;
          }
        },
        _ => serde_json::from_value(value).map_err(serde::de::Error::custom)?,
      };
      any_map.insert(key, any_value);
    }

    if let Some(err) = error {
      Err(err)
    } else {
      Ok(AnyMap(Arc::new(any_map)))
    }
  }
}
impl<'de> Deserialize<'de> for AnyMap {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: Deserializer<'de>,
  {
    deserializer.deserialize_map(AnyMapVisitor)
  }
}
impl Deref for AnyMap {
  type Target = HashMap<String, Any>;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl DerefMut for AnyMap {
  fn deref_mut(&mut self) -> &mut Self::Target {
    Arc::make_mut(&mut self.0)
  }
}

/// Builder for [AnyMap].
#[derive(Default)]
pub struct AnyMapBuilder {
  inner: AnyMap,
}

impl AnyMapBuilder {
  pub fn new() -> Self {
    Self::default()
  }

  /// Insert the Any into the map.
  /// Sometimes you need a integer or a float into the map, you should use [insert_i64_value] or
  /// [insert_f64_value]. Because the integer value will be treated as a float value when calling
  /// this method.
  pub fn insert_any<K: AsRef<str>>(mut self, key: K, value: impl Into<Any>) -> Self {
    let key = key.as_ref();
    self.inner.insert(key.to_string(), value.into());
    self
  }

  pub fn insert_maps<K: AsRef<str>, T: Into<AnyMap>>(mut self, key: K, items: Vec<T>) -> Self {
    self.inner.insert_array(key, items);
    self
  }

  pub fn insert_str_value<K: AsRef<str>, S: ToString>(mut self, key: K, s: S) -> Self {
    self.inner.insert_str_value(key, s.to_string());
    self
  }

  pub fn insert_bool_value<K: AsRef<str>>(mut self, key: K, value: bool) -> Self {
    self.inner.insert_bool_value(key, value);
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

  pub fn insert<K: AsRef<str>>(&mut self, key: K, value: impl Into<Any>) {
    let key = key.as_ref();
    self.map_ref.insert_with_txn(self.txn, key, value.into());
  }

  pub fn update<K: AsRef<str>>(self, key: K, value: AnyMap) -> Self {
    let key = key.as_ref();
    let field_setting_map = self.map_ref.get_or_create_map_with_txn(self.txn, key);
    value.fill_map_ref(self.txn, &field_setting_map);

    self
  }

  pub fn remove<K: AsRef<str>>(self, key: K) -> Self {
    let key = key.as_ref();
    self.map_ref.remove(self.txn, key);
    self
  }
}
