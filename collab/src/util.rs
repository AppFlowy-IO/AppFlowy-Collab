/*
pub fn insert_json_value_to_map_ref(
  key: &str,
  value: &Value,
  map_ref: MapRef,
  txn: &mut TransactionMut,
) {
  if value.is_object() {
    value
      .as_object()
      .unwrap()
      .into_iter()
      .for_each(|(key, inner_value)| {
        let new_map_ref = if inner_value.is_object() {
          map_ref.insert(txn, key.as_str(), MapPrelim::<Any>::new());
          map_ref
            .get(txn, key)
            .map(|value| value.cast::<MapRef>().unwrap())
            .unwrap()
        } else {
          map_ref.clone()
        };
        insert_json_value_to_map_ref(key, inner_value, new_map_ref, txn);
      });
  } else if value.is_array() {
    map_ref.insert(txn, key, ArrayPrelim::<Vec<Any>, Any>::from(vec![]));
    let array_ref = map_ref
      .get(txn, key)
      .map(|value| value.cast::<ArrayRef>().unwrap())
      .unwrap();
    insert_json_value_to_array_ref(txn, &array_ref, value);
  } else {
    match json_value_to_any(value.clone()) {
      Ok(value) => {
        map_ref.insert(txn, key, value);
      },
      Err(e) => tracing::error!("🔴{:?}", e),
    }
  }
}

pub fn insert_json_value_to_array_ref(
  txn: &mut TransactionMut,
  array_ref: &ArrayRef,
  value: &Value,
) {
  // Only support string
  let values = value.as_array().unwrap();
  let values = values
    .iter()
    .flat_map(|value| value.as_str())
    .collect::<Vec<_>>();

  for value in values {
    array_ref.push_back(txn, value);
  }
}

pub fn json_value_to_any(json_value: Value) -> Result<Any> {
  let value = serde_json::from_value(json_value)?;
  Ok(value)
}

pub fn any_to_json_value(any: Any) -> Result<Value> {
  let json_value = serde_json::to_value(&any)?;
  Ok(json_value)
}*/

use serde::de::DeserializeOwned;
use serde::Serialize;
use std::sync::Arc;

use crate::core::collab::Path;
use crate::core::value::Entity;
use crate::error::CollabError;
use yrs::block::Prelim;
use yrs::branch::BranchPtr;
use yrs::types::ToJson;
use yrs::{Any, ArrayPrelim, ArrayRef, Map, MapPrelim, MapRef, ReadTxn, TransactionMut, Out};

pub trait MapExt: Map {
  #[inline]
  fn as_map(&self) -> MapRef {
    MapRef::from(BranchPtr::from(self.as_ref()))
  }

  fn get_with_txn<T, V>(&self, txn: &T, key: &str) -> Option<V>
  where
    T: ReadTxn,
    V: TryFrom<Out, Error = Out>,
  {
    let value = self.get(txn, key)?;
    V::try_from(value).ok()
  }

  fn get_or_init_map<S: Into<Arc<str>>>(&self, txn: &mut TransactionMut, key: S) -> MapRef {
    let key = key.into();
    match self.get(txn, &key) {
      Some(Out::YMap(map)) => map,
      _ => self.insert(txn, key, MapPrelim::default()),
    }
  }

  fn get_or_init_array<S: Into<Arc<str>>>(&self, txn: &mut TransactionMut, key: S) -> ArrayRef {
    let key = key.into();
    match self.get(txn, &key) {
      Some(Out::YArray(array)) => array,
      _ => self.insert(txn, key, ArrayPrelim::default()),
    }
  }

  #[inline]
  fn get_with_path<P, T, V>(&self, txn: &T, path: P) -> Option<V>
  where
    P: Into<Path>,
    T: ReadTxn,
    V: TryFrom<Out, Error = Out>,
  {
    let value = self.get_value_with_path(txn, path)?;
    value.cast::<V>().ok()
  }

  fn get_value_with_path<P, T>(&self, txn: &T, path: P) -> Option<Out>
  where
    P: Into<Path>,
    T: ReadTxn,
  {
    let mut current = self.as_map();
    let mut path = path.into();
    let last = path.pop()?;
    for field in path {
      current = current.get(txn, &field)?.cast().ok()?;
    }
    current.get(txn, &last)
  }

  fn insert_json_with_path<P, V>(
    &self,
    txn: &mut TransactionMut,
    path: P,
    value: V,
  ) -> Result<(), CollabError>
  where
    P: Into<Path>,
    V: Serialize,
  {
    let value = serde_json::to_value(value)?;
    self.insert_with_path(txn, path, Entity::from(value))?;
    Ok(())
  }

  fn get_json_with_path<T, P, V>(&self, txn: &T, path: P) -> Result<V, CollabError>
  where
    T: ReadTxn,
    P: Into<Path>,
    V: DeserializeOwned,
  {
    let value = self
      .get_value_with_path(txn, path)
      .ok_or(CollabError::UnexpectedEmpty(
        "value not found on path".to_string(),
      ))?;
    let value = serde_json::to_value(value.to_json(txn))?;
    Ok(serde_json::from_value(value)?)
  }

  fn insert_with_path<P, V>(
    &self,
    txn: &mut TransactionMut,
    path: P,
    value: V,
  ) -> Result<V::Return, CollabError>
  where
    P: Into<Path>,
    V: Prelim,
  {
    let mut current = self.as_map();
    let mut path = path.into();
    let last = match path.pop() {
      Some(field) => field,
      None => return Err(CollabError::NoRequiredData("empty path".into())),
    };
    for field in path {
      current = match current.get(txn, &field) {
        None => current.insert(txn, field, MapPrelim::default()),
        Some(value) => value
          .cast()
          .map_err(|_| CollabError::NoRequiredData(field))?,
      };
    }
    Ok(current.insert(txn, last, value))
  }

  fn remove_with_path<P>(&self, txn: &mut TransactionMut<'_>, path: P) -> Option<Out>
  where
    P: Into<Path>,
  {
    let mut path = path.into();
    if path.is_empty() {
      return None;
    }
    let last = path.pop()?;
    let mut current = self.as_map();
    for field in path {
      current = current.get(txn, &field)?.cast().ok()?;
    }
    current.remove(txn, &last)
  }
}

impl<T: Map + Into<MapRef>> MapExt for T {}

macro_rules! create_deserialize_numeric {
  ($type:ty, $visitor_name:ident, $deserialize_fn_name:ident) => {
    pub fn $deserialize_fn_name<'de, D>(deserializer: D) -> Result<$type, D::Error>
    where
      D: serde::Deserializer<'de>,
    {
      struct $visitor_name;

      impl<'de> serde::de::Visitor<'de> for $visitor_name {
        type Value = $type;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
          formatter.write_str(concat!("a numeric type convertible to ", stringify!($type)))
        }

        // Implement visit methods for various numeric types

        fn visit_u8<E>(self, value: u8) -> Result<$type, E> {
          Ok(value as $type)
        }

        fn visit_u16<E>(self, value: u16) -> Result<$type, E> {
          Ok(value as $type)
        }

        fn visit_u32<E>(self, value: u32) -> Result<$type, E> {
          Ok(value as $type)
        }

        fn visit_u64<E>(self, value: u64) -> Result<$type, E>
        where
          E: serde::de::Error,
        {
          <$type>::try_from(value).map_err(E::custom)
        }

        fn visit_i32<E>(self, value: i32) -> Result<$type, E> {
          Ok(value as $type)
        }

        fn visit_i64<E>(self, value: i64) -> Result<$type, E>
        where
          E: serde::de::Error,
        {
          <$type>::try_from(value).map_err(E::custom)
        }

        fn visit_f64<E>(self, value: f64) -> Result<$type, E>
        where
          E: serde::de::Error,
        {
          if value.fract() == 0.0 && value >= <$type>::MIN as f64 && value <= <$type>::MAX as f64 {
            Ok(value as $type)
          } else {
            Err(E::custom(concat!(
              "f64 value cannot be accurately represented as ",
              stringify!($type)
            )))
          }
        }

        fn visit_f32<E>(self, value: f32) -> Result<$type, E>
        where
          E: serde::de::Error,
        {
          if value.fract() == 0.0 && value >= <$type>::MIN as f32 && value <= <$type>::MAX as f32 {
            Ok(value as $type)
          } else {
            Err(E::custom(concat!(
              "f32 value cannot be accurately represented as ",
              stringify!($type)
            )))
          }
        }
      }
      deserializer.deserialize_any($visitor_name)
    }
  };
}

// Create deserialization functions for i32 and i64
create_deserialize_numeric!(i32, I32Visitor, deserialize_i32_from_numeric);
create_deserialize_numeric!(i64, I64Visitor, deserialize_i64_from_numeric);
