use std::error::Error;

pub trait KV {
  type Range: Iterator<Item = Self::Entry>;
  type Entry: KVEntry;
  type Value: AsRef<[u8]>;
  type Error: Error;

  /// Get a value by key
  fn get(&self, key: &[u8]) -> Result<Option<Self::Value>, Self::Error>;

  /// Insert a key to a new value, returning the last value if it exists
  fn insert(&self, key: &[u8], value: &[u8]) -> Result<Option<Self::Value>, Self::Error>;

  /// Remove a key, returning the last value if it exists
  fn remove(&self, key: &[u8]) -> Result<(), Self::Error>;

  /// Remove all keys in the range [from, to]
  fn remove_range(&self, from: &[u8], to: &[u8]) -> Result<(), Self::Error>;

  /// Return an iterator over the range of keys
  fn iter_range(&self, from: &[u8], to: &[u8]) -> Result<Self::Range, Self::Error>;

  /// Return the entry prior to the given key
  fn next_back_entry(&self, key: &[u8]) -> Result<Option<Self::Entry>, Self::Error>;
}

/// A key-value entry
pub trait KVEntry {
  fn key(&self) -> &[u8];
  fn value(&self) -> &[u8];
}
