use crate::core::any_map::AnyMap;
use crate::core::array_wrapper::ArrayRefExtension;
use crate::preclude::{MapRefExtension, YrsValue};

use std::ops::{Deref, DerefMut};
use yrs::types::Value;
use yrs::{Array, ArrayRef, ReadTxn, TransactionMut};

/// A wrapper around an `ArrayRef` that allows to store `AnyMap` in it.
pub struct ArrayMap(pub Vec<AnyMap>);

impl ArrayMap {
  pub fn new() -> Self {
    Self(Vec::new())
  }

  pub fn from_any_maps(items: Vec<AnyMap>) -> Self {
    let mut this = Self::new();
    for item in items {
      this.push(item);
    }
    this
  }

  pub fn from_array_ref<R: ReadTxn>(txn: &R, array_ref: &ArrayRef) -> Self {
    let mut any_array = Self::new();
    for value in array_ref.iter(txn) {
      match value {
        Value::YMap(map_ref) => {
          any_array.push(AnyMap::from((txn, &map_ref)));
        },
        _ => debug_assert!(false, "Unsupported type"),
      }
    }
    any_array
  }

  pub fn extend_array_ref(self, txn: &mut TransactionMut, array_ref: ArrayRef) {
    for value in self.0 {
      let map_ref = array_ref.insert_map_with_txn(txn);
      value.fill_map_ref(txn, &map_ref);
    }
  }
}

impl Deref for ArrayMap {
  type Target = Vec<AnyMap>;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl DerefMut for ArrayMap {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.0
  }
}

pub struct ArrayMapUpdate<'a, 'b> {
  array_ref: ArrayRef,
  txn: &'a mut TransactionMut<'b>,
}

impl<'a, 'b> ArrayMapUpdate<'a, 'b> {
  pub fn new(txn: &'a mut TransactionMut<'b>, array_ref: ArrayRef) -> Self {
    Self { txn, array_ref }
  }

  pub fn insert(self, any_map: AnyMap, index: u32) -> Self {
    let map_ref = self.array_ref.insert_map_at_index_with_txn(self.txn, index);
    any_map.fill_map_ref(self.txn, &map_ref);
    self
  }

  pub fn push(self, any_map: AnyMap) -> Self {
    let map_ref = self.array_ref.insert_map_with_txn(self.txn);
    any_map.fill_map_ref(self.txn, &map_ref);
    self
  }

  pub fn remove(self, id: &str) -> Self {
    if let Some(pos) = self.index_of(id) {
      self.array_ref.remove(self.txn, pos as u32);
    }
    self
  }

  pub fn update<'c, F>(self, id: &str, f: F) -> Self
  where
    F: FnOnce(AnyMap) -> AnyMap,
  {
    if let Some(pos) = self.index_of(id) {
      let pos = pos as u32;
      if let YrsValue::YMap(map_ref) = self.array_ref.get(self.txn, pos).unwrap() {
        let any_map = AnyMap::from_map_ref(self.txn, &map_ref);
        f(any_map).fill_map_ref(self.txn, &map_ref);
      }
    }

    self
  }

  fn index_of(&self, id: &str) -> Option<u32> {
    self
      .array_ref
      .iter(self.txn)
      .position(|v| {
        if let YrsValue::YMap(map_ref) = v {
          if let Some(target_id) = map_ref.get_str_with_txn(self.txn, "id") {
            return target_id == id;
          }
        }
        false
      })
      .map(|v| v as u32)
  }
}
