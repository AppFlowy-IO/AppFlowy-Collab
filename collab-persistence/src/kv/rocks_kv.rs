use std::ops;
use std::ops::RangeBounds;
use std::path::Path;
use std::sync::Arc;

use rocksdb::Direction::Forward;
use rocksdb::{
  ColumnFamilyDescriptor, DBIteratorWithThreadMode, Direction, ErrorKind, IteratorMode, Options,
  ReadOptions, SingleThreaded, Transaction, TransactionDB, TransactionDBOptions,
  TransactionOptions, WriteOptions,
};

use crate::kv::{KVEntry, KVStore};
use crate::PersistenceError;

pub type RocksCollabDB = RocksStore;

#[derive(Clone)]
pub struct RocksStore {
  db: Arc<TransactionDB>,
}

impl RocksStore {
  /// Open a new RocksDB database at the given path.
  /// If the database is corrupted, try to repair it. If it cannot be repaired, return an error.
  pub fn open(path: impl AsRef<Path>) -> Result<Self, PersistenceError> {
    let txn_db_opts = TransactionDBOptions::default();
    let mut db_opts = Options::default();
    db_opts.create_if_missing(true);
    let db = match TransactionDB::<SingleThreaded>::open(&db_opts, &txn_db_opts, &path) {
      Ok(db) => Ok(db),
      Err(e) => {
        tracing::error!("🔴open collab db error: {:?}", e);
        match e.kind() {
          // A few types of corruption that repair may be able to fix:
          // 1. Missing files: If SST files or other vital files have been accidentally deleted or
          // are missing due to a filesystem error, the repair function can often recover the database
          // to a usable state.
          // 2. Truncated files: If a file is truncated due to a crash or filesystem error, the repair
          // function might be able to recover the database.
          // 3. Incorrect file sizes: If the size of a file on disk is different from what RocksDB
          // expects (like the "Sst file size mismatch" error), the repair function might be able
          // to correct this.
          ErrorKind::Corruption | ErrorKind::Unknown => {
            // If the database is corrupted, try to repair it
            tracing::info!("Trying to repair collab database");
            match TransactionDB::<SingleThreaded>::repair(&db_opts, &path) {
              Ok(_) => tracing::info!("Collab database repaired"),
              Err(e) => tracing::error!("🔴Failed to repair collab database: {:?}", e),
            }
          },
          _ => {},
        }
        TransactionDB::<SingleThreaded>::open(&db_opts, &txn_db_opts, &path)
      },
    }?;
    Ok(Self { db: Arc::new(db) })
  }

  pub fn open_with_cfs(
    names: Vec<String>,
    path: impl AsRef<Path>,
  ) -> Result<Self, PersistenceError> {
    let txn_db_opts = TransactionDBOptions::default();
    let mut db_opts = Options::default();
    db_opts.create_if_missing(true);
    db_opts.create_missing_column_families(true);

    // CFs
    let cf_opts = Options::default();
    let cfs = names
      .into_iter()
      .map(|name| ColumnFamilyDescriptor::new(name, cf_opts.clone()))
      .collect::<Vec<_>>();
    let db = Arc::new(TransactionDB::open_cf_descriptors(
      &db_opts,
      &txn_db_opts,
      path,
      cfs,
    )?);
    Ok(Self { db })
  }

  /// Return a read transaction that accesses the database exclusively.
  pub fn read_txn(&self) -> RocksKVStoreImpl<'_, TransactionDB> {
    let mut txn_options = TransactionOptions::default();
    txn_options.set_snapshot(true);
    let txn = self
      .db
      .transaction_opt(&WriteOptions::default(), &txn_options);
    MutexRocksKVStoreImpl::new(txn)
  }

  pub fn write_txn(&self) -> RocksKVStoreImpl<'_, TransactionDB> {
    let mut txn_options = TransactionOptions::default();
    txn_options.set_snapshot(true);
    let txn = self
      .db
      .transaction_opt(&WriteOptions::default(), &txn_options);
    MutexRocksKVStoreImpl::new(txn)
  }

  /// Create a write transaction that accesses the database exclusively.
  /// The transaction will be committed when the closure [F] returns.
  pub fn with_write_txn<F, O>(&self, f: F) -> Result<O, PersistenceError>
  where
    F: FnOnce(&RocksKVStoreImpl<'_, TransactionDB>) -> Result<O, PersistenceError>,
  {
    let mut txn_options = TransactionOptions::default();
    // Use snapshot to provides a consistent view of the data. This snapshot can then be used
    // to perform read operations, and the returned data will be consistent with the database
    // state at the time the snapshot was created, regardless of any subsequent modifications
    // made by other transactions.
    txn_options.set_snapshot(true);
    let txn = self
      .db
      .transaction_opt(&WriteOptions::default(), &txn_options);
    let store = MutexRocksKVStoreImpl::new(txn);
    let result = f(&store)?;
    store.0.commit()?;
    Ok(result)
  }
}

/// Implementation of [KVStore] for [RocksStore]. This is a wrapper around [Transaction].
// pub struct RocksKVStoreImpl<'a, DB: Send + Sync>(Transaction<'a, DB>);
pub type RocksKVStoreImpl<'a, DB> = MutexRocksKVStoreImpl<'a, DB>;
pub struct MutexRocksKVStoreImpl<'a, DB: Send>(Transaction<'a, DB>);

unsafe impl<'db, DB: Send> Send for MutexRocksKVStoreImpl<'db, DB> {}

impl<'a, DB: Send + Sync> MutexRocksKVStoreImpl<'a, DB> {
  pub fn new(txn: Transaction<'a, DB>) -> Self {
    Self(txn)
  }

  pub fn commit_transaction(self) -> Result<(), PersistenceError> {
    self.0.commit()?;
    Ok(())
  }
}

impl<'a, DB: Send + Sync> KVStore<'a> for MutexRocksKVStoreImpl<'a, DB> {
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
    let iter = self.0.iterator_opt(iterator_mode, opt);
    Ok(RocksDBRange {
      // Safe to transmute because the lifetime of the iterator is the same as the lifetime of the
      // transaction.
      inner: unsafe { std::mem::transmute(iter) },
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

impl<'a, DB: Send + Sync> From<Transaction<'a, DB>> for RocksKVStoreImpl<'a, DB> {
  #[inline(always)]
  fn from(txn: Transaction<'a, DB>) -> Self {
    MutexRocksKVStoreImpl::new(txn)
  }
}

// impl<'a, DB: Send + Sync> From<RocksKVStoreImpl<'a, DB>> for Transaction<'a, DB> {
//   fn from(store: RocksKVStoreImpl<'a, DB>) -> Self {
//     store.0.lock()
//   }
// }

// impl<'a, DB: Send + Sync> Deref for RocksKVStoreImpl<'a, DB> {
//   type Target = Transaction<'a, DB>;
//
//   #[inline(always)]
//   fn deref(&self) -> &Self::Target {
//     &self.0
//   }
// }

pub type RocksDBVec = Vec<u8>;

pub struct RocksDBRange<'a, DB> {
  inner: DBIteratorWithThreadMode<'a, Transaction<'a, DB>>,
  to: Vec<u8>,
}

impl<'a, DB: Send + Sync> Iterator for RocksDBRange<'a, DB> {
  type Item = RocksDBEntry;

  fn next(&mut self) -> Option<Self::Item> {
    let n = self.inner.next()?;
    if let Ok((key, value)) = n {
      if key.as_ref() >= self.to.as_slice() {
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
