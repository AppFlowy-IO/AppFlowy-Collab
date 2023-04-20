use crate::kv::{KVEntry, KV};
use std::ops::RangeBounds;
use std::sync::Arc;

use crate::PersistenceError;
use sled::{Batch, Db, IVec, Iter};

#[derive(Clone)]
pub struct SledKV(pub Arc<Db>);

impl SledKV {
  pub fn new(db: Db) -> Self {
    Self(Arc::new(db))
  }
}

impl KV for SledKV {
  type Range = SledRange;
  type Entry = SledEntry;
  type Value = IVec;
  type Error = PersistenceError;

  fn get(&self, key: &[u8]) -> Result<Option<Self::Value>, Self::Error> {
    let value = self.0.get(key)?;
    Ok(value)
  }

  /// Insert a key to a new value, returning the last value if it exists
  fn insert<K: AsRef<[u8]>, V: AsRef<[u8]>>(
    &self,
    key: K,
    value: V,
  ) -> Result<Option<Self::Value>, Self::Error> {
    let old_value = self.0.insert(key.as_ref(), value.as_ref())?;
    Ok(old_value)
  }

  fn remove(&self, key: &[u8]) -> Result<(), Self::Error> {
    self.0.remove(key)?;
    Ok(())
  }

  fn remove_range(&self, from: &[u8], to: &[u8]) -> Result<(), Self::Error> {
    let mut batch = Batch::default();
    let iter = self.0.range(from..=to);
    for key in iter {
      let key = key?.0;
      batch.remove(key);
    }
    self.0.apply_batch(batch)?;
    Ok(())
  }

  fn range<K: AsRef<[u8]>, R: RangeBounds<K>>(&self, range: R) -> Self::Range {
    let iter = self.0.range(range);
    SledRange(iter)
  }

  fn next_back_entry(&self, key: &[u8]) -> Result<Option<Self::Entry>, Self::Error> {
    match self.0.range(..key).next_back() {
      Some(Ok((k, v))) => Ok(Some(SledEntry { key: k, value: v })),
      _ => Ok(None),
    }
  }
}

pub struct SledRange(Iter);

impl Iterator for SledRange {
  type Item = SledEntry;

  fn next(&mut self) -> Option<Self::Item> {
    let (k, v) = self.0.next()?.ok()?;
    Some(SledEntry { key: k, value: v })
  }
}

pub struct SledEntry {
  key: IVec,
  value: IVec,
}

impl KVEntry for SledEntry {
  fn key(&self) -> &[u8] {
    self.key.as_ref()
  }

  fn value(&self) -> &[u8] {
    self.value.as_ref()
  }
}
