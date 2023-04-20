pub mod kv_sled_impl;

use crate::PersistenceError;

use std::ops::RangeBounds;
use std::sync::Arc;

pub trait KV: Send + Sync {
  type Range: Iterator<Item = Self::Entry>;
  type Entry: KVEntry;
  type Value: AsRef<[u8]>;
  type Error: Into<PersistenceError>;

  /// Get a value by key
  fn get(&self, key: &[u8]) -> Result<Option<Self::Value>, Self::Error>;

  /// Insert a key to a new value, returning the last value if it exists
  fn insert<K: AsRef<[u8]>, V: AsRef<[u8]>>(
    &self,
    key: K,
    value: V,
  ) -> Result<Option<Self::Value>, Self::Error>;

  /// Remove a key, returning the last value if it exists
  fn remove(&self, key: &[u8]) -> Result<(), Self::Error>;

  /// Remove all keys in the range [from, to]
  fn remove_range(&self, from: &[u8], to: &[u8]) -> Result<(), Self::Error>;

  /// Return an iterator over the range of keys
  fn range<K: AsRef<[u8]>, R: RangeBounds<K>>(&self, range: R) -> Self::Range;

  /// Return the entry prior to the given key
  fn next_back_entry(&self, key: &[u8]) -> Result<Option<Self::Entry>, Self::Error>;
}

/// A key-value entry
pub trait KVEntry {
  fn key(&self) -> &[u8];
  fn value(&self) -> &[u8];
}

impl<T> KV for Arc<T>
where
  T: KV,
{
  type Range = <T as KV>::Range;
  type Entry = <T as KV>::Entry;
  type Value = <T as KV>::Value;
  type Error = <T as KV>::Error;

  fn get(&self, key: &[u8]) -> Result<Option<Self::Value>, Self::Error> {
    (**self).get(key)
  }

  fn insert<K: AsRef<[u8]>, V: AsRef<[u8]>>(
    &self,
    key: K,
    value: V,
  ) -> Result<Option<Self::Value>, Self::Error> {
    (**self).insert(key, value)
  }

  fn remove(&self, key: &[u8]) -> Result<(), Self::Error> {
    (**self).remove(key)
  }

  fn remove_range(&self, from: &[u8], to: &[u8]) -> Result<(), Self::Error> {
    (**self).remove_range(from, to)
  }

  fn range<K: AsRef<[u8]>, R: RangeBounds<K>>(&self, range: R) -> Self::Range {
    self.as_ref().range(range)
  }

  fn next_back_entry(&self, key: &[u8]) -> Result<Option<Self::Entry>, Self::Error> {
    (**self).next_back_entry(key)
  }
}
