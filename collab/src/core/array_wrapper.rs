use crate::preclude::{CollabContext, MapRefWrapper, YrsValue};
use crate::util::insert_json_value_to_array_ref;
use anyhow::Result;
use lib0::any::Any;
use serde::Serialize;
use std::ops::{Deref, DerefMut};
use yrs::block::Prelim;
use yrs::{Array, ArrayRef, MapPrelim, ReadTxn, Transact, Transaction, TransactionMut};

#[derive(Clone)]
pub struct ArrayRefWrapper {
  array_ref: ArrayRef,
  collab_ctx: CollabContext,
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

  pub fn get(&self, index: u32) -> Option<YrsValue> {
    let txn = self.transact();
    self.array_ref.get(&txn, index)
  }

  pub fn get_with_txn<T: ReadTxn>(&self, txn: &T, index: u32) -> Option<YrsValue> {
    self.array_ref.get(txn, index)
  }

  pub fn insert_with_txn<V: Prelim>(&self, txn: &mut TransactionMut, index: u32, value: V) {
    self.array_ref.insert(txn, index, value);
  }

  pub fn push_with_txn<V: Prelim>(&self, txn: &mut TransactionMut, value: V) {
    self.array_ref.push_back(txn, value);
  }

  pub fn push_json_with_txn<T: Serialize>(&self, txn: &mut TransactionMut, value: T) -> Result<()> {
    let value = serde_json::to_value(value)?;
    insert_json_value_to_array_ref(txn, &self.array_ref, &value);
    Ok(())
  }

  pub fn create_map_ref(&self) -> MapRefWrapper {
    self.with_transact_mut(|txn| self.create_map_with_txn(txn))
  }

  pub fn create_map_with_txn(&self, txn: &mut TransactionMut) -> MapRefWrapper {
    let array = MapPrelim::<Any>::new();
    let map_ref = self.array_ref.push_back(txn, array);
    MapRefWrapper::new(map_ref, self.collab_ctx.clone())
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
  pub fn remove_with_txn(&self, txn: &mut TransactionMut, index: u32) {
    self.array_ref.remove(txn, index);
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
