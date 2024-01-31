use crate::core::value::YrsValueExtension;
use anyhow::Result;

use serde::{de, Deserializer};
use serde_json::Value as JsonValue;
use std::fmt;
use std::future::Future;

use yrs::{Any, Array, ArrayPrelim, ArrayRef, Map, MapPrelim, MapRef, TransactionMut};

pub fn insert_json_value_to_map_ref(
  key: &str,
  value: &JsonValue,
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
            .map(|value| value.to_ymap().cloned().unwrap())
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
      .map(|value| value.to_yarray().cloned().unwrap())
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
  value: &JsonValue,
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

pub fn json_value_to_any(json_value: JsonValue) -> Result<Any> {
  let value = serde_json::from_value(json_value)?;
  Ok(value)
}

pub fn any_to_json_value(any: Any) -> Result<JsonValue> {
  let json_value = serde_json::to_value(&any)?;
  Ok(json_value)
}

macro_rules! create_deserialize_numeric {
  ($type:ty, $visitor_name:ident, $deserialize_fn_name:ident) => {
    pub fn $deserialize_fn_name<'de, D>(deserializer: D) -> Result<$type, D::Error>
    where
      D: Deserializer<'de>,
    {
      struct $visitor_name;

      impl<'de> de::Visitor<'de> for $visitor_name {
        type Value = $type;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
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
          E: de::Error,
        {
          <$type>::try_from(value).map_err(E::custom)
        }

        fn visit_i32<E>(self, value: i32) -> Result<$type, E> {
          Ok(value as $type)
        }

        fn visit_i64<E>(self, value: i64) -> Result<$type, E>
        where
          E: de::Error,
        {
          <$type>::try_from(value).map_err(E::custom)
        }

        fn visit_f64<E>(self, value: f64) -> Result<$type, E>
        where
          E: de::Error,
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
          E: de::Error,
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

#[cfg(target_arch = "wasm32")]
pub fn af_spawn<T>(future: T) -> tokio::task::JoinHandle<T::Output>
where
  T: Future + 'static,
  T::Output: 'static,
{
  tokio::task::spawn_local(future)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn af_spawn<T>(future: T) -> tokio::task::JoinHandle<T::Output>
where
  T: Future + Send + 'static,
  T::Output: Send + 'static,
{
  tokio::spawn(future)
}
