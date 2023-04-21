use std::ops;
use std::ops::{Deref, RangeBounds};
use std::path::Path;
use std::sync::Arc;

use rocksdb::Direction::Forward;
use rocksdb::{
  DBIteratorWithThreadMode, Direction, IteratorMode, ReadOptions, Transaction, TransactionDB,
};

use crate::kv::{KVEntry, KVStore};
use crate::PersistenceError;

pub type RocksCollabDB = RocksKVStore;

#[derive(Clone)]
pub struct RocksKVStore {
  db: Arc<TransactionDB>,
}

impl RocksKVStore {
  pub fn open(path: impl AsRef<Path>) -> Result<Self, PersistenceError> {
    let db = Arc::new(TransactionDB::open_default(path)?);
    Ok(Self { db })
  }

  pub fn read_txn(&self) -> RocksKVStoreImpl<'_, TransactionDB> {
    let txn = self.db.transaction();
    RocksKVStoreImpl(txn)
  }

  pub fn with_write_txn<F, O>(&self, f: F) -> Result<O, PersistenceError>
  where
    F: FnOnce(&RocksKVStoreImpl<'_, TransactionDB>) -> Result<O, PersistenceError>,
  {
    let txn = self.db.transaction();
    let store = RocksKVStoreImpl(txn);
    let result = f(&store)?;
    store.0.commit()?;
    Ok(result)
  }
}

pub struct RocksKVStoreImpl<'a, DB>(Transaction<'a, DB>);

impl<'a, DB> KVStore<'a> for RocksKVStoreImpl<'a, DB> {
  type Range = RocksDBRange<'a, DB>;
  type Entry = RocksDBEntry;
  type Value = RocksDBVec;
  type Error = PersistenceError;

  fn get<K: AsRef<[u8]>>(&self, key: K) -> Result<Option<Self::Value>, Self::Error> {
    if let Some(value) = self.0.get(key)? {
      Ok(Some(value))
    } else {
      Ok(None)
    }
  }

  fn insert<K: AsRef<[u8]>, V: AsRef<[u8]>>(&self, key: K, value: V) -> Result<(), Self::Error> {
    self.0.put(key, value)?;
    Ok(())
  }

  fn remove(&self, key: &[u8]) -> Result<(), Self::Error> {
    self.0.delete(key)?;
    Ok(())
  }

  fn remove_range(&self, from: &[u8], to: &[u8]) -> Result<(), Self::Error> {
    let mut opt = ReadOptions::default();
    opt.set_iterate_lower_bound(from);
    opt.set_iterate_upper_bound(to);
    let i = self
      .0
      .iterator_opt(IteratorMode::From(from, Direction::Forward), opt);
    for res in i {
      let (key, _) = res?;
      self.0.delete(key)?;
    }
    Ok(())
  }

  fn range<K: AsRef<[u8]>, R: RangeBounds<K>>(&self, range: R) -> Result<Self::Range, Self::Error> {
    let mut opt = ReadOptions::default();
    let mut from: &[u8] = &[];
    let mut to: &[u8] = &[];
    match range.start_bound() {
      ops::Bound::Included(start) => {
        from = start.as_ref();
        opt.set_iterate_lower_bound(start.as_ref());
      },
      ops::Bound::Excluded(start) => {
        from = start.as_ref();
        opt.set_iterate_lower_bound(start.as_ref());
      },
      ops::Bound::Unbounded => {},
    };

    match range.end_bound() {
      ops::Bound::Included(end) => {
        opt.set_iterate_upper_bound(end.as_ref());
        to = end.as_ref();
      },
      ops::Bound::Excluded(end) => {
        opt.set_iterate_upper_bound(end.as_ref());
        to = end.as_ref();
      },
      ops::Bound::Unbounded => {},
    };
    let iterator_mode = IteratorMode::From(from, Forward);
    let raw = self.0.iterator_opt(iterator_mode, opt);
    Ok(RocksDBRange {
      inner: unsafe { std::mem::transmute(raw) },
      to: to.to_vec(),
    })
  }

  fn next_back_entry(&self, key: &[u8]) -> Result<Option<Self::Entry>, Self::Error> {
    let opt = ReadOptions::default();
    let mut raw = self.0.raw_iterator_opt(opt);
    raw.seek_for_prev(key);
    if let Some((key, value)) = raw.item() {
      Ok(Some(RocksDBEntry::new(key.to_vec(), value.to_vec())))
    } else {
      Ok(None)
    }
  }
}

impl<'a, DB> From<Transaction<'a, DB>> for RocksKVStoreImpl<'a, DB> {
  #[inline(always)]
  fn from(txn: Transaction<'a, DB>) -> Self {
    RocksKVStoreImpl(txn)
  }
}

impl<'a, DB> From<RocksKVStoreImpl<'a, DB>> for Transaction<'a, DB> {
  fn from(store: RocksKVStoreImpl<'a, DB>) -> Self {
    store.0
  }
}

impl<'a, DB> Deref for RocksKVStoreImpl<'a, DB> {
  type Target = Transaction<'a, DB>;

  #[inline(always)]
  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

pub type RocksDBVec = Vec<u8>;

pub struct RocksDBRange<'a, DB> {
  inner: DBIteratorWithThreadMode<'a, Transaction<'a, DB>>,
  to: Vec<u8>,
}

impl<'a, DB> Iterator for RocksDBRange<'a, DB> {
  type Item = RocksDBEntry;

  fn next(&mut self) -> Option<Self::Item> {
    let n = self.inner.next()?;
    if let Ok((key, value)) = n {
      if key.as_ref() >= &self.to {
        None
      } else {
        Some(RocksDBEntry::new(key.to_vec(), value.to_vec()))
      }
    } else {
      None
    }
  }
}

pub struct RocksDBEntry {
  key: Vec<u8>,
  value: Vec<u8>,
}

impl RocksDBEntry {
  pub fn new(key: Vec<u8>, value: Vec<u8>) -> Self {
    Self { key, value }
  }
}

impl KVEntry for RocksDBEntry {
  fn key(&self) -> &[u8] {
    self.key.as_ref()
  }

  fn value(&self) -> &[u8] {
    self.value.as_ref()
  }
}
