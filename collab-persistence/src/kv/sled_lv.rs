use crate::kv::{KVEntry, KVRange, KVStore};
use parking_lot::RwLock;
use std::marker::PhantomData;
use std::ops::{Deref, RangeBounds};
use std::path::Path;
use std::sync::Arc;

use crate::{CollabDB, PersistenceError};
use sled::{Batch, Db, IVec, Iter};

pub type SledCollabDB = SledKVStore;

#[derive(Clone)]
pub struct SledKVStore(pub Arc<RwLock<Db>>);

impl SledKVStore {
  pub fn open(path: impl AsRef<Path>) -> Result<Self, PersistenceError> {
    let db = sled::open(path)?;
    let store = SledKVStore::new(db);
    Ok(store)
  }

  pub fn new(db: Db) -> Self {
    Self(Arc::new(RwLock::new(db)))
  }

  pub fn kv_store_impl(&self) -> SledKVStoreImpl {
    SledKVStoreImpl(self.0.clone())
  }
}

pub struct SledKVStoreImpl(Arc<RwLock<Db>>);

impl Deref for SledKVStoreImpl {
  type Target = Arc<RwLock<Db>>;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl KVStore<'static> for SledKVStoreImpl {
  type Range = SledRange;
  type Entry = SledEntry;
  type Value = IVec;
  type Error = PersistenceError;

  fn get<K: AsRef<[u8]>>(&self, key: K) -> Result<Option<Self::Value>, Self::Error> {
    let value = self.0.read().get(key)?;
    Ok(value)
  }

  /// Insert a key to a new value, returning the last value if it exists
  fn insert<K: AsRef<[u8]>, V: AsRef<[u8]>>(&self, key: K, value: V) -> Result<(), Self::Error> {
    let _ = self.0.write().insert(key.as_ref(), value.as_ref())?;
    Ok(())
  }

  fn remove(&self, key: &[u8]) -> Result<(), Self::Error> {
    self.0.write().remove(key)?;
    Ok(())
  }

  fn remove_range(&self, from: &[u8], to: &[u8]) -> Result<(), Self::Error> {
    let mut batch = Batch::default();
    let iter = self.0.write().range(from..=to);
    for key in iter {
      let key = key?.0;
      batch.remove(key);
    }
    self.0.write().apply_batch(batch)?;
    Ok(())
  }

  fn range<K: AsRef<[u8]>, R: RangeBounds<K>>(&self, range: R) -> Result<Self::Range, Self::Error> {
    let iter = self.0.read().range(range);
    Ok(SledRange(iter))
  }

  fn next_back_entry(&self, key: &[u8]) -> Result<Option<Self::Entry>, Self::Error> {
    match self.0.read().range(..key).next_back() {
      Some(Ok((k, v))) => Ok(Some(SledEntry { key: k, value: v })),
      _ => Ok(None),
    }
  }

  fn commit(self) -> Result<(), Self::Error> {
    Ok(())
  }
}

// fn range2<'a, K, R, RI>(&self, range: R) -> Result<RI, Self::Error>
// where
//   K: AsRef<[u8]>,
//   R: RangeBounds<K>,
//   RI: KVRange<'a, Range = Self::Range, Entry = Self::Entry, Error = Self::Error>,
// {
//   let kv = SledKVRange {
//     db: &self.0,
//     range,
//     phantom: Default::default(),
//   };
//   Ok(kv)
// }

pub struct SledKVRange<'a, K: AsRef<[u8]>, B: RangeBounds<K>> {
  db: &'a Arc<Db>,
  range: B,
  phantom: PhantomData<K>,
}

impl<'a, K, B> KVRange<'a> for SledKVRange<'a, K, B>
where
  K: AsRef<[u8]>,
  B: RangeBounds<K>,
{
  type Range = SledRange;
  type Entry = SledEntry;
  type Error = PersistenceError;

  fn kv_range(self) -> Result<Self::Range, Self::Error> {
    let iter = self.db.range(self.range);
    Ok(SledRange(iter))
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
