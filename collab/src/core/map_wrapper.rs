use std::ops::{Deref, DerefMut};

use lib0::any::Any;
use serde::de::DeserializeOwned;
use serde::Serialize;
use yrs::block::Prelim;
use yrs::types::{ToJson, Value};
use yrs::{
  ArrayPrelim, ArrayRef, Map, MapPrelim, MapRef, ReadTxn, TextPrelim, Transaction, TransactionMut,
};

use crate::core::array_wrapper::ArrayRefWrapper;
use crate::core::text_wrapper::TextRefWrapper;
use crate::preclude::*;
use crate::util::lib0_any_to_json_value;

pub trait CustomMapRef {
  fn from_map_ref(map_ref: MapRefWrapper) -> Self;
}

impl CustomMapRef for MapRefWrapper {
  fn from_map_ref(map_ref: MapRefWrapper) -> Self {
    map_ref
  }
}

#[derive(Clone)]
pub struct MapRefWrapper {
  map_ref: MapRef,
  pub collab_ctx: CollabContext,
}

impl MapRefWrapper {
  pub fn new(map_ref: MapRef, collab_ctx: CollabContext) -> Self {
    Self {
      collab_ctx,
      map_ref,
    }
  }

  pub fn into_inner(self) -> MapRef {
    self.map_ref
  }

  pub fn insert<V: Prelim>(&self, key: &str, value: V) {
    self.collab_ctx.with_transact_mut(|txn| {
      self.map_ref.insert(txn, key, value);
    })
  }

  pub fn insert_with_txn<V: Prelim>(&self, txn: &mut TransactionMut, key: &str, value: V) {
    self.map_ref.insert(txn, key, value);
  }

  pub fn insert_text_with_txn(&self, txn: &mut TransactionMut, key: &str) -> TextRefWrapper {
    let text = TextPrelim::new("");
    let text_ref = self.map_ref.insert(txn, key, text);
    TextRefWrapper::new(text_ref, self.collab_ctx.clone())
  }

  pub fn insert_array<V: Prelim>(&self, key: &str, values: Vec<V>) -> ArrayRefWrapper {
    self.with_transact_mut(|txn| self.insert_array_with_txn(txn, key, values))
  }

  pub fn insert_map<T: Into<MapPrelim<lib0Any>>>(&self, key: &str, value: T) {
    self.with_transact_mut(|txn| self.insert_map_with_txn(txn, key, value));
  }

  pub fn insert_map_with_txn<T: Into<MapPrelim<lib0Any>>>(
    &self,
    txn: &mut TransactionMut,
    key: &str,
    value: T,
  ) {
    self.map_ref.insert(txn, key, value.into());
  }

  pub fn insert_array_with_txn<V: Prelim>(
    &self,
    txn: &mut TransactionMut,
    key: &str,
    values: Vec<V>,
  ) -> ArrayRefWrapper {
    let array = self.map_ref.insert(txn, key, ArrayPrelim::from(values));
    ArrayRefWrapper::new(array, self.collab_ctx.clone())
  }

  pub fn create_array_if_not_exist_with_txn<V: Prelim>(
    &self,
    txn: &mut TransactionMut,
    key: &str,
    values: Vec<V>,
  ) -> ArrayRefWrapper {
    let array_ref = self
      .map_ref
      .create_array_if_not_exist_with_txn(txn, key, values);
    ArrayRefWrapper::new(array_ref, self.collab_ctx.clone())
  }

  pub fn create_map_with_txn_if_not_exist(
    &self,
    txn: &mut TransactionMut,
    key: &str,
  ) -> MapRefWrapper {
    let map_ref = self.map_ref.create_map_if_not_exist_with_txn(txn, key);
    MapRefWrapper::new(map_ref, self.collab_ctx.clone())
  }

  pub fn get_or_insert_array_with_txn<V: Prelim>(
    &self,
    txn: &mut TransactionMut,
    key: &str,
  ) -> ArrayRefWrapper {
    self
      .get_array_ref_with_txn(txn, key)
      .unwrap_or_else(|| self.insert_array_with_txn::<V>(txn, key, vec![]))
  }

  pub fn create_map_with_txn(&self, txn: &mut TransactionMut, key: &str) -> MapRefWrapper {
    let map = MapPrelim::<lib0::any::Any>::new();
    let map_ref = self.map_ref.insert(txn, key, map);
    MapRefWrapper::new(map_ref, self.collab_ctx.clone())
  }

  pub fn get_map_with_txn<T: ReadTxn>(&self, txn: &T, key: &str) -> Option<MapRefWrapper> {
    let a = self.map_ref.get(txn, key);
    if let Some(Value::YMap(map_ref)) = a {
      return Some(MapRefWrapper::new(map_ref, self.collab_ctx.clone()));
    }
    None
  }

  pub fn get_array_ref_with_txn<T: ReadTxn>(&self, txn: &T, key: &str) -> Option<ArrayRefWrapper> {
    let array_ref = self
      .map_ref
      .get(txn, key)
      .map(|value| value.to_yarray())??;
    Some(ArrayRefWrapper::new(array_ref, self.collab_ctx.clone()))
  }

  pub fn get_text_ref_with_txn<T: ReadTxn>(&self, txn: &T, key: &str) -> Option<TextRefWrapper> {
    let text_ref = self.map_ref.get(txn, key).map(|value| value.to_ytext())??;
    Some(TextRefWrapper::new(text_ref, self.collab_ctx.clone()))
  }

  pub fn insert_json<T: Serialize>(&self, key: &str, value: T) {
    let value = serde_json::to_value(&value).unwrap();
    self.collab_ctx.with_transact_mut(|txn| {
      insert_json_value_to_map_ref(key, &value, self.map_ref.clone(), txn);
    });
  }

  pub fn insert_json_with_txn<T: Serialize>(&self, txn: &mut TransactionMut, key: &str, value: T) {
    let value = serde_json::to_value(&value).unwrap();
    if let Some(map_ref) = self.get_map_with_txn(txn, key) {
      insert_json_value_to_map_ref(key, &value, map_ref.into_inner(), txn);
    }
  }

  pub fn get_json<T: DeserializeOwned>(&self, key: &str) -> Option<T> {
    self.get_json_with_txn(&self.collab_ctx.transact(), key)
  }

  pub fn get_json_with_txn<T: DeserializeOwned>(&self, txn: &Transaction, key: &str) -> Option<T> {
    let map_ref = self.get_map_with_txn(txn, key)?;
    let json_value = lib0_any_to_json_value(map_ref.into_inner().to_json(txn)).ok()?;
    serde_json::from_value::<T>(json_value).ok()
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

  pub fn to_json_value(&self) -> JsonValue {
    let txn = self.collab_ctx.transact();
    serde_json::to_value(&self.map_ref.to_json(&txn)).unwrap()
  }

  pub fn to_json(&self) -> String {
    let txn = self.collab_ctx.transact();
    let value = self.map_ref.to_json(&txn);
    let mut json_str = String::new();
    value.to_json(&mut json_str);
    json_str
  }
}

impl MapRefExtension for MapRefWrapper {
  fn map_ref(&self) -> &MapRef {
    &self.map_ref
  }
}

/// A trait defining an interface for interacting with a map reference.
/// It provides various methods to insert and retrieve different data types to/from the map,
/// leveraging transactions for maintaining consistency.
pub trait MapRefExtension {
  fn map_ref(&self) -> &MapRef;

  fn create_array_with_txn<V: Prelim>(
    &self,
    txn: &mut TransactionMut,
    key: &str,
    values: Vec<V>,
  ) -> ArrayRef {
    self.map_ref().insert(txn, key, ArrayPrelim::from(values))
  }

  fn insert_with_txn<V: Prelim>(&self, txn: &mut TransactionMut, key: &str, value: V) {
    self.map_ref().insert(txn, key, value);
  }

  fn insert_str_with_txn<T: ToString>(&self, txn: &mut TransactionMut, key: &str, value: T) {
    self.map_ref().insert(
      txn,
      key,
      lib0Any::String(value.to_string().into_boxed_str()),
    );
  }

  fn insert_i64_with_txn<T: Into<i64>>(&self, txn: &mut TransactionMut, key: &str, value: T) {
    self
      .map_ref()
      .insert(txn, key, lib0Any::BigInt(value.into()));
  }

  fn insert_f64_with_txn(&self, txn: &mut TransactionMut, key: &str, value: f64) {
    self.map_ref().insert(txn, key, lib0Any::Number(value));
  }

  fn insert_bool_with_txn(&self, txn: &mut TransactionMut, key: &str, value: bool) {
    self.map_ref().insert(txn, key, lib0Any::Bool(value));
  }

  fn create_map_with_txn(&self, txn: &mut TransactionMut, key: &str) -> MapRef {
    let map = MapPrelim::<lib0::any::Any>::new();
    self.map_ref().insert(txn, key, map)
  }

  fn get_array_ref_with_txn<T: ReadTxn>(&self, txn: &T, key: &str) -> Option<ArrayRef> {
    self
      .map_ref()
      .get(txn, key)
      .map(|value| value.to_yarray())?
  }

  fn get_map_with_txn<T: ReadTxn>(&self, txn: &T, key: &str) -> Option<MapRef> {
    self.map_ref().get(txn, key).map(|value| value.to_ymap())?
  }

  /// Retrieves a reference to a map associated with the given key from the underlying datastore,
  /// or inserts a new, empty map if no map is associated with the key. This function is transactional,
  /// meaning the changes made within the function won't be applied until the transaction is committed.
  /// If the transaction is rolled back, the changes will be discarded.
  ///
  /// # Arguments
  /// * `txn` - A mutable reference to a transaction that is currently in progress.
  /// * `key` - The key associated with the map in the datastore.
  ///
  /// # Returns
  /// A reference to the map. If a map was already associated with the given key, the returned reference is to the existing map.
  /// Otherwise, it references the newly inserted empty map.
  ///
  fn get_or_create_map_with_txn(&self, txn: &mut TransactionMut, key: &str) -> MapRef {
    self
      .get_map_with_txn(txn, key)
      .unwrap_or_else(|| self.create_map_with_txn(txn, key))
  }

  /// Inserts a map into the underlying datastore if a map with the provided key does not already exist.
  /// This function is transactional, meaning the changes made within the function won't be
  /// applied until the transaction is committed. If the transaction is rolled back, the changes will be discarded.
  ///
  /// This function checks whether a map already exists associated with the given key. If the key does
  /// not exist, it will create a new map and insert it into the datastore. If a map already exists,
  /// it will not perform any insert operation.
  ///
  /// # Arguments
  /// * `txn` - A mutable reference to a transaction that is currently in progress.
  /// * `key` - The key to associate with the map in the datastore.
  ///
  fn create_map_if_not_exist_with_txn(&self, txn: &mut TransactionMut, key: &str) -> MapRef {
    match self
      .map_ref()
      .get(txn, key)
      .and_then(|value| value.to_ymap())
    {
      None => self
        .map_ref()
        .insert(txn, key, MapPrelim::<lib0::any::Any>::new()),
      Some(map_ref) => map_ref,
    }
  }

  /// Retrieves a reference to an array associated with the given key from the underlying datastore,
  /// or inserts a new, empty array if no array is associated with the key. This function is transactional,
  /// meaning the changes made within the function won't be applied until the transaction is committed.
  /// If the transaction is rolled back, the changes will be discarded.
  ///
  /// # Type Parameters
  /// * `V` - The type of elements stored in the array. `V` must implement `Prelim` trait.
  ///
  /// # Arguments
  /// * `txn` - A mutable reference to a transaction that is currently in progress.
  /// * `key` - The key associated with the array in the datastore.
  fn get_or_create_array_with_txn<V: Prelim>(
    &self,
    txn: &mut TransactionMut,
    key: &str,
  ) -> ArrayRef {
    self
      .get_array_ref_with_txn(txn, key)
      .unwrap_or_else(|| self.create_array_with_txn::<V>(txn, key, vec![]))
  }

  /// Inserts an array into the underlying datastore if an array with the provided key does not already exist.
  /// This function is transactional, meaning the changes made within the function won't be applied
  /// until the transaction is committed. If the transaction is rolled back, the changes will be discarded.
  ///
  /// This function checks whether an array already exists associated with the given key. If the key does not exist, it will create a new array and insert it into the datastore. If an array already exists, it will not perform any insert operation.
  ///
  /// # Type Parameters
  /// * `V` - The type of elements stored in the array. `V` must implement `Prelim` trait.
  ///
  /// # Arguments
  /// * `txn` - A mutable reference to a transaction that is currently in progress.
  /// * `key` - The key to associate with the array in the datastore.
  /// * `values` - The values to be stored in the array if a new array needs to be created.
  ///
  fn create_array_if_not_exist_with_txn<V: Prelim>(
    &self,
    txn: &mut TransactionMut,
    key: &str,
    values: Vec<V>,
  ) -> ArrayRef {
    match self
      .map_ref()
      .get(txn, key)
      .and_then(|value| value.to_yarray())
    {
      None => self.map_ref().insert(txn, key, ArrayPrelim::from(values)),
      Some(array_ref) => array_ref,
    }
  }

  fn get_str_with_txn<T: ReadTxn>(&self, txn: &T, key: &str) -> Option<String> {
    if let Some(Value::Any(Any::String(value))) = self.map_ref().get(txn, key) {
      return Some(value.to_string());
    }
    None
  }

  fn get_text_ref_with_txn<T: ReadTxn>(&self, txn: &T, key: &str) -> Option<TextRef> {
    self.map_ref().get(txn, key).map(|value| value.to_ytext())?
  }

  fn get_i64_with_txn<T: ReadTxn>(&self, txn: &T, key: &str) -> Option<i64> {
    if let Some(Value::Any(Any::BigInt(value))) = self.map_ref().get(txn, key) {
      return Some(value);
    }
    None
  }

  fn get_f64_with_txn<T: ReadTxn>(&self, txn: &T, key: &str) -> Option<f64> {
    if let Some(Value::Any(Any::Number(value))) = self.map_ref().get(txn, key) {
      return Some(value);
    }
    None
  }

  fn get_bool_with_txn<T: ReadTxn>(&self, txn: &T, key: &str) -> Option<bool> {
    if let Some(Value::Any(Any::Bool(value))) = self.map_ref().get(txn, key) {
      return Some(value);
    }
    None
  }

  fn delete_with_txn(&self, txn: &mut TransactionMut, key: &str) {
    self.map_ref().remove(txn, key);
  }
}

impl MapRefExtension for MapRef {
  fn map_ref(&self) -> &MapRef {
    self
  }
}

impl Deref for MapRefWrapper {
  type Target = MapRef;

  fn deref(&self) -> &Self::Target {
    &self.map_ref
  }
}

impl DerefMut for MapRefWrapper {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.map_ref
  }
}
