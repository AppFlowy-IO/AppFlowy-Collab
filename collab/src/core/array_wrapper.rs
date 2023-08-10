use std::ops::{Deref, DerefMut};

use anyhow::Result;
use lib0::any::Any;
use serde::Serialize;
use yrs::block::Prelim;
use yrs::{Array, ArrayRef, MapPrelim, MapRef, ReadTxn, Transact, Transaction, TransactionMut};

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
    let txn = self.array_ref.transact();
    self.to_map_refs_with_txn(&txn)
  }

  pub fn to_map_refs_with_txn<T: ReadTxn>(&self, txn: &T) -> Vec<MapRefWrapper> {
    self
      .array_ref
      .iter(txn)
      .flat_map(|value| value.to_ymap())
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

  fn insert_map_with_txn(&self, txn: &mut TransactionMut) -> MapRef {
    let array = MapPrelim::<Any>::new();
    self.array_ref().push_back(txn, array)
  }

  fn insert_map_at_index_with_txn(&self, txn: &mut TransactionMut, index: u32) -> MapRef {
    let array = MapPrelim::<Any>::new();
    self.array_ref().insert(txn, index, array)
  }

  fn remove_with_id(&self, txn: &mut TransactionMut, key: &str, id: &str) {
    if let Some(index) = self
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
    {
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
