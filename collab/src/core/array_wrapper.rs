use std::ops::{Deref, DerefMut};

use crate::core::value::YrsValueExtension;
use anyhow::Result;
use serde::Serialize;
use yrs::block::Prelim;
use yrs::{Array, ArrayRef, MapPrelim, MapRef, ReadTxn, Transaction, TransactionMut};

use crate::preclude::{CollabContext, MapRefExtension, MapRefWrapper, YrsValue};
use crate::util::insert_json_value_to_array_ref;

#[derive(Clone)]
pub struct ArrayRefWrapper {
  array_ref: ArrayRef,
  pub collab_ctx: CollabContext,
}

impl ArrayRefWrapper {
  pub fn new(array_ref: ArrayRef, collab_ctx: CollabContext) -> Self {
    Self {
      array_ref,
      collab_ctx,
    }
  }

  pub fn transact(&self) -> Transaction {
    self.collab_ctx.transact()
  }

  pub fn with_transact_mut<F, T>(&self, f: F) -> T
  where
    F: FnOnce(&mut TransactionMut) -> T,
  {
    self.collab_ctx.with_transact_mut(f)
  }

  pub fn push<V: Prelim>(&self, value: V) {
    self.with_transact_mut(|txn| {
      self.array_ref.push_back(txn, value);
    });
  }

  pub fn push_json_with_txn<T: Serialize>(&self, txn: &mut TransactionMut, value: T) -> Result<()> {
    let value = serde_json::to_value(value)?;
    insert_json_value_to_array_ref(txn, &self.array_ref, &value);
    Ok(())
  }

  pub fn to_map_refs(&self) -> Vec<MapRefWrapper> {
    let txn = self.collab_ctx.transact();
    self.to_map_refs_with_txn(&txn)
  }

  pub fn to_map_refs_with_txn<T: ReadTxn>(&self, txn: &T) -> Vec<MapRefWrapper> {
    self
      .array_ref
      .iter(txn)
      .flat_map(|value| value.to_ymap().cloned())
      .map(|map_ref| MapRefWrapper::new(map_ref, self.collab_ctx.clone()))
      .collect::<Vec<_>>()
  }

  pub fn remove_with_txn(&self, txn: &mut TransactionMut, index: u32) -> Option<YrsValue> {
    let value = self.array_ref.get(txn, index);
    self.array_ref.remove(txn, index);
    value
  }

  pub fn into_inner(self) -> ArrayRef {
    self.array_ref
  }
}

impl Deref for ArrayRefWrapper {
  type Target = ArrayRef;

  fn deref(&self) -> &Self::Target {
    &self.array_ref
  }
}

impl DerefMut for ArrayRefWrapper {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.array_ref
  }
}

pub trait ArrayRefExtension {
  fn array_ref(&self) -> &ArrayRef;

  fn insert_map_with_txn(&self, txn: &mut TransactionMut, value: Option<MapPrelim>) -> MapRef {
    let value = value.unwrap_or_default();
    self.array_ref().push_back(txn, value)
  }

  fn insert_map_at_index_with_txn(
    &self,
    txn: &mut TransactionMut,
    index: u32,
    value: Option<MapPrelim>,
  ) -> MapRef {
    let value = value.unwrap_or_default();
    self.array_ref().insert(txn, index, value)
  }

  fn mut_map_element_with_txn<'a, F, R>(
    &'a self,
    txn: &'a mut TransactionMut,
    id: &str,
    key: &str,
    f: F,
  ) where
    F: FnOnce(&mut TransactionMut, &MapRef) -> Option<R>,
    R: Prelim,
  {
    if let Some(index) = self.position_with_txn(txn, id, key) {
      if let Some(YrsValue::YMap(map)) = self.array_ref().get(txn, index) {
        if let Some(new_value) = f(txn, &map) {
          self.array_ref().remove(txn, index);
          self.array_ref().insert(txn, index, new_value);
        }
      }
    }
  }

  /// Retrieves the position of an element in the underlying data structure
  /// based on its ID and key.
  ///
  /// The `position_with_txn` method searches through the data structure and returns
  /// the position (as a `u32`) of the element that matches the given ID and key.
  /// If no match is found, the method returns `None`.
  ///
  /// The method specifically looks for elements of type `YrsValue::YMap` and checks
  /// if the map contains the specified key with the corresponding ID value.
  ///
  /// # Parameters
  /// - `txn`: A reference to a transaction object for reading.
  /// - `id`: A string slice representing the ID value to match.
  /// - `key`: A string slice representing the key to look up in the `YMap`.
  ///
  /// # Returns
  /// - `Option<u32>`: The position of the matching element as a `u32` if found, or `None` otherwise.
  ///
  fn position_with_txn<T: ReadTxn>(&self, txn: &T, id: &str, key: &str) -> Option<u32> {
    self
      .array_ref()
      .iter(txn)
      .position(|value| {
        if let YrsValue::YMap(map) = value {
          map
            .get_str_with_txn(txn, key)
            .map(|value| value.as_str() == id)
            .unwrap_or(false)
        } else {
          false
        }
      })
      .map(|index| index as u32)
  }

  fn remove_with_id(&self, txn: &mut TransactionMut, id: &str, key: &str) {
    if let Some(index) = self.position_with_txn(txn, id, key) {
      self.array_ref().remove(txn, index);
    }
  }

  fn clear(&self, txn: &mut TransactionMut) {
    let len = self.array_ref().len(txn);
    self.array_ref().remove_range(txn, 0, len);
  }
}

impl ArrayRefExtension for ArrayRef {
  fn array_ref(&self) -> &ArrayRef {
    self
  }
}
