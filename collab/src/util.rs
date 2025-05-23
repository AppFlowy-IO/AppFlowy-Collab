use anyhow::Result;
use std::collections::HashMap;

use serde::Serialize;
use serde::de::DeserializeOwned;
use std::sync::Arc;

use crate::core::collab::Path;
use crate::core::value::Entity;
use crate::error::CollabError;
use crate::preclude::{Collab, FillRef, JsonValue};
use yrs::block::Prelim;
use yrs::branch::BranchPtr;
use yrs::types::text::YChange;
use yrs::types::{DefaultPrelim, Delta, ToJson};
use yrs::updates::decoder::Decode;
use yrs::{
  Any, Array, ArrayPrelim, ArrayRef, Map, MapPrelim, MapRef, Out, ReadTxn, StateVector, Text,
  TextPrelim, TextRef, TransactionMut, Update,
};

pub trait MapExt: Map {
  #[inline]
  fn as_map(&self) -> MapRef {
    MapRef::from(BranchPtr::from(self.as_ref()))
  }

  fn get_id(&self, txn: &impl ReadTxn) -> Option<Arc<str>> {
    let out = self.get(txn, "id")?;
    if let Out::Any(Any::String(str)) = out {
      Some(str)
    } else {
      None
    }
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

  fn get_or_init_text<S: Into<Arc<str>>>(&self, txn: &mut TransactionMut, key: S) -> TextRef {
    let key = key.into();
    match self.get(txn, &key) {
      Some(Out::YText(text)) => text,
      _ => self.insert(txn, key, TextPrelim::new("")),
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

impl MapExt for MapRef {}

pub trait TextExt: Text {
  fn delta<T: ReadTxn>(&self, tx: &T) -> Vec<Delta> {
    let changes = self.diff(tx, YChange::identity);
    let mut deltas = vec![];
    for change in changes {
      let delta = Delta::Inserted(change.insert, change.attributes);
      deltas.push(delta);
    }
    deltas
  }
}

impl TextExt for TextRef {}

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

pub fn json_value_to_any(json_value: JsonValue) -> Result<Any> {
  let value = serde_json::from_value(json_value)?;
  Ok(value)
}

pub fn any_to_json_value(any: Any) -> Result<JsonValue> {
  let json_value = serde_json::to_value(&any)?;
  Ok(json_value)
}

// Create deserialization functions for i32 and i64
create_deserialize_numeric!(i32, I32Visitor, deserialize_i32_from_numeric);
create_deserialize_numeric!(i64, I64Visitor, deserialize_i64_from_numeric);

pub trait ArrayExt: Array {
  fn clear(&self, txn: &mut TransactionMut) {
    let len = self.len(txn);
    self.remove_range(txn, 0, len);
  }

  /// Removes the first element that satisfies the predicate.
  fn remove_one<F, V>(&self, txn: &mut TransactionMut, predicate: F)
  where
    F: Fn(V) -> bool,
    V: TryFrom<Out>,
  {
    let mut i = 0;
    while let Some(out) = self.get(txn, i) {
      if let Ok(value) = V::try_from(out) {
        if predicate(value) {
          self.remove(txn, i);
          break;
        }
      }
      i += 1;
    }
  }

  fn update_map<F>(&self, txn: &mut TransactionMut, id: &str, f: F)
  where
    F: FnOnce(&mut HashMap<String, Any>),
  {
    let map_ref: MapRef = self.upsert(txn, id);
    let mut map = map_ref.to_json(txn).into_map().unwrap();
    f(&mut map);
    Any::from(map).fill(txn, &map_ref).unwrap();
  }

  fn index_by_id<T: ReadTxn>(&self, txn: &T, id: &str) -> Option<u32> {
    let i = self.iter(txn).position(|value| {
      if let Ok(value) = value.cast::<MapRef>() {
        if let Some(current_id) = value.get_id(txn) {
          return &*current_id == id;
        }
      }
      false
    })?;
    Some(i as u32)
  }

  fn upsert<V>(&self, txn: &mut TransactionMut, id: &str) -> V
  where
    V: DefaultPrelim + TryFrom<Out>,
  {
    match self.index_by_id(txn, id) {
      None => self.push_back(txn, V::default_prelim()),
      Some(i) => {
        let out = self.get(txn, i).unwrap();
        match V::try_from(out) {
          Ok(shared_ref) => shared_ref,
          Err(_) => {
            self.remove(txn, i);
            self.push_back(txn, V::default_prelim())
          },
        }
      },
    }
  }
}

impl<T> ArrayExt for T where T: Array {}

pub trait AnyExt {
  fn into_map(self) -> Option<HashMap<String, Any>>;
  fn into_array(self) -> Option<Vec<Any>>;
}

impl AnyExt for Any {
  fn into_map(self) -> Option<HashMap<String, Any>> {
    match self {
      Any::Map(map) => Arc::into_inner(map),
      _ => None,
    }
  }

  fn into_array(self) -> Option<Vec<Any>> {
    match self {
      Any::Array(array) => Some(array.to_vec()),
      _ => None,
    }
  }
}

pub trait AnyMapExt {
  fn get_as<V>(&self, key: &str) -> Option<V>
  where
    V: TryFrom<Any, Error = Any>;
}

impl AnyMapExt for HashMap<String, Any> {
  fn get_as<V>(&self, key: &str) -> Option<V>
  where
    V: TryFrom<Any, Error = Any>,
  {
    let value = self.get(key)?.clone();
    value.cast().ok()
  }
}

impl AnyMapExt for Any {
  fn get_as<V>(&self, key: &str) -> Option<V>
  where
    V: TryFrom<Any, Error = Any>,
  {
    match self {
      Any::Map(map) => map.get_as(key),
      _ => None,
    }
  }
}

pub fn is_change_since_sv(collab: &Collab, state_vector: &StateVector) -> bool {
  let txn = collab.transact();
  let update = txn.encode_state_as_update_v1(state_vector);
  let update = Update::decode_v1(&update).unwrap();

  !update.state_vector().is_empty() || !update.delete_set().is_empty()
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::core::collab::default_client_id;

  #[tokio::test]
  async fn test_no_changes_after_initialization() {
    let collab = Collab::new(1, "1", "1", default_client_id());
    let sv_1 = collab.transact().state_vector();
    assert!(
      !is_change_since_sv(&collab, &sv_1),
      "There should be no changes after initialization."
    );
  }

  #[tokio::test]
  async fn test_insert_triggers_change() {
    let mut collab = Collab::new(1, "1", "1", default_client_id());
    let sv_1 = collab.transact().state_vector();

    collab.insert("text", "hello world");
    assert!(
      is_change_since_sv(&collab, &sv_1),
      "Insert operation should trigger a change."
    );
  }

  #[tokio::test]
  async fn test_no_changes_after_state_vector_update() {
    let mut collab = Collab::new(1, "1", "1", default_client_id());
    collab.insert("text", "hello world");
    let sv_2 = collab.transact().state_vector();

    // No changes since the last state vector (sv_2)
    assert!(
      !is_change_since_sv(&collab, &sv_2),
      "There should be no changes after state vector update."
    );
  }

  #[tokio::test]
  async fn test_remove_triggers_change() {
    let mut collab = Collab::new(1, "1", "1", default_client_id());
    collab.insert("text", "hello world");
    let sv_1 = collab.transact().state_vector();

    collab.remove("text");
    assert!(
      is_change_since_sv(&collab, &sv_1),
      "Remove operation should trigger a change."
    );
  }

  #[tokio::test]
  async fn test_multiple_operations_trigger_change() {
    let mut collab = Collab::new(1, "1", "1", default_client_id());
    let sv_1 = collab.transact().state_vector();

    collab.insert("text", "hello");
    collab.insert("text", " world");
    collab.remove("text");

    assert!(
      is_change_since_sv(&collab, &sv_1),
      "Multiple operations should trigger a change."
    );
  }

  #[tokio::test]
  async fn test_empty_insert_and_remove_no_change() {
    let mut collab = Collab::new(1, "1", "1", default_client_id());
    let sv_1 = collab.transact().state_vector();

    // Perform empty insert and remove operations
    collab.insert("text", "");
    collab.remove("text");

    assert!(is_change_since_sv(&collab, &sv_1));
  }

  #[tokio::test]
  async fn test_changes_after_sequence_of_operations() {
    let mut collab = Collab::new(1, "1", "1", default_client_id());
    let sv_1 = collab.transact().state_vector();

    collab.insert("text", "hello");
    assert!(
      is_change_since_sv(&collab, &sv_1),
      "First insert should trigger a change."
    );

    let sv_2 = collab.transact().state_vector();
    collab.remove("text");
    assert!(
      is_change_since_sv(&collab, &sv_2),
      "Remove operation should trigger a change after insert."
    );
  }

  #[tokio::test]
  async fn test_changes_after_full_update() {
    let mut collab = Collab::new(1, "1", "1", default_client_id());
    collab.insert("text", "data");
    let sv_1 = collab.transact().state_vector();

    collab.insert("text", " more data");
    collab.remove("text");
    let update = collab.transact().encode_state_as_update_v1(&sv_1);

    assert!(
      !update.is_empty(),
      "The update should not be empty after changes."
    );
    assert!(
      is_change_since_sv(&collab, &sv_1),
      "Changes should be detected after insert and remove."
    );
  }
}
