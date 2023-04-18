use crate::kv::kv::{KVEntry, KV};

use sled::{Batch, Db, IVec, Iter};

pub struct SledKV {
  db: Db,
}

impl KV for SledKV {
  type Range = SledRange;
  type Entry = SledEntry;
  type Value = IVec;
  type Error = sled::Error;

  fn get(&self, key: &[u8]) -> Result<Option<Self::Value>, Self::Error> {
    self.db.get(key)
  }

  /// Insert a key to a new value, returning the last value if it exists
  fn insert(&self, key: &[u8], value: &[u8]) -> Result<Option<Self::Value>, Self::Error> {
    self.db.insert(key, value)
  }

  fn remove(&self, key: &[u8]) -> Result<(), Self::Error> {
    self.db.remove(key)?;
    Ok(())
  }

  fn remove_range(&self, from: &[u8], to: &[u8]) -> Result<(), Self::Error> {
    let mut batch = Batch::default();
    let iter = self.db.range(from..=to);
    for key in iter {
      let key = key?.0;
      batch.remove(key);
    }
    self.db.apply_batch(batch)?;
    Ok(())
  }

  fn iter_range(&self, from: &[u8], to: &[u8]) -> Result<Self::Range, Self::Error> {
    let iter = self.db.range(from..=to);
    Ok(SledRange(iter))
  }

  fn next_back_entry(&self, key: &[u8]) -> Result<Option<Self::Entry>, Self::Error> {
    match self.db.range(..key).next_back() {
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
