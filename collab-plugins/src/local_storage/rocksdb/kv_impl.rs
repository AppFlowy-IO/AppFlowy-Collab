use std::ops;
use std::ops::RangeBounds;
use std::path::Path;
use std::sync::Arc;

use crate::local_storage::kv::doc::CollabKVAction;
use crate::local_storage::kv::{KVEntry, KVStore, PersistenceError};
use rocksdb::Direction::Forward;
use rocksdb::{
  DBIteratorWithThreadMode, Direction, ErrorKind, IteratorMode, Options, ReadOptions,
  SingleThreaded, Transaction, TransactionDB, TransactionDBOptions, TransactionOptions,
  WriteOptions,
};

#[derive(Clone)]
pub struct RocksStore {
  db: Arc<TransactionDB>,
}

impl RocksStore {
  /// Open a new RocksDB database at the given path.
  /// If the database is corrupted, try to repair it. If it cannot be repaired, return an error.
  pub fn open_opt(path: impl AsRef<Path>, auto_repair: bool) -> Result<Self, PersistenceError> {
    let txn_db_opts = TransactionDBOptions::default();
    let mut db_opts = Options::default();
    // This option sets the upper limit for the total number of background jobs (both flushes and compactions)
    // that can run concurrently. If you set this value too low, you might limit the ability of RocksDB to
    // efficiently flush and compact data, potentially leading to increased write latency or larger disk space usage.
    // On the other hand, setting it too high could lead to excessive CPU and I/O usage, impacting the overall
    // performance of the system.
    db_opts.set_max_background_jobs(4);

    // sst
    db_opts.set_max_open_files(50);

    // compression
    db_opts.set_compression_type(rocksdb::DBCompressionType::Zstd);
    db_opts.set_blob_compression_type(rocksdb::DBCompressionType::Zstd);
    db_opts.set_compaction_style(rocksdb::DBCompactionStyle::Level);

    // wal
    // Can't set the wal because existing rocksdb databases don't have the wal directory
    // It might cause data lost.
    // db_opts.set_wal_dir(path.as_ref().join("wal"));

    db_opts.set_wal_bytes_per_sync(1024 * 1024);
    db_opts.set_wal_size_limit_mb(2);
    db_opts.set_max_total_wal_size(20 * 1024 * 1024);

    // write buffer
    db_opts.set_bytes_per_sync(1024 * 1024);
    db_opts.set_write_buffer_size(2 * 1024 * 1024);
    db_opts.set_max_write_buffer_number(2);
    db_opts.set_min_write_buffer_number_to_merge(1);

    // level 0
    db_opts.set_level_zero_file_num_compaction_trigger(2);
    db_opts.set_level_zero_slowdown_writes_trigger(5);
    db_opts.set_level_zero_stop_writes_trigger(10);

    // log
    db_opts.set_recycle_log_file_num(5);
    db_opts.set_keep_log_file_num(5);
    db_opts.set_db_log_dir(path.as_ref().join("logs"));
    db_opts.create_if_missing(true);

    let open_result = TransactionDB::<SingleThreaded>::open(&db_opts, &txn_db_opts, &path);
    let db = match open_result {
      Ok(db) => {
        //
        Ok(db)
      },
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
            if auto_repair {
              // If the database is corrupted, try to repair it
              // tracing::info!("Trying to repair collab database");
              TransactionDB::<SingleThreaded>::repair(&db_opts, &path).map_err(|err| {
                PersistenceError::RocksdbRepairFail(format!(
                  "Failed to repair collab database: {:?}",
                  err
                ))
              })?;
              TransactionDB::<SingleThreaded>::open(&db_opts, &txn_db_opts, &path).map_err(|err| {
                PersistenceError::RocksdbRepairFail(format!(
                  "Failed to repair collab database: {:?}",
                  err
                ))
              })
            } else {
              Err(PersistenceError::RocksdbCorruption(e.to_string()))
            }
          },
          _ => Err(e.into()),
        }
      },
    }?;

    Ok(Self { db: Arc::new(db) })
  }

  /// Open a new RocksDB database at the given path.
  /// If the database is corrupted, try to repair it. If it cannot be repaired, return an error.
  pub fn open(path: impl AsRef<Path>) -> Result<Self, PersistenceError> {
    Self::open_opt(path, false)
  }

  pub fn flush(&self) -> Result<(), PersistenceError> {
    Ok(())
  }

  /// Return a read transaction that accesses the database exclusively.
  pub fn read_txn(&self) -> impl CollabKVAction<'_, Error = PersistenceError> {
    let mut txn_options = TransactionOptions::default();
    // Use snapshot to provides a consistent view of the data. This snapshot can then be used
    // to perform read operations, and the returned data will be consistent with the database
    // state at the time the snapshot was created, regardless of any subsequent modifications
    // made by other transactions.
    txn_options.set_snapshot(true);
    let txn = self
      .db
      .transaction_opt(&WriteOptions::default(), &txn_options);
    RocksKVStoreImpl::new(txn)
  }

  /// Create a write transaction that accesses the database exclusively.
  /// The transaction will be committed when the closure [F] returns.
  pub fn with_write_txn<F, O>(&self, f: F) -> Result<O, PersistenceError>
  where
    F: FnOnce(&RocksKVStoreImpl<'_, TransactionDB>) -> Result<O, PersistenceError>,
  {
    let txn_options = TransactionOptions::default();
    let txn = self
      .db
      .transaction_opt(&WriteOptions::default(), &txn_options);
    let store = RocksKVStoreImpl::new(txn);
    let result = f(&store)?;
    store.0.commit()?;
    Ok(result)
  }
}

/// Implementation of [KVStore] for [RocksStore]. This is a wrapper around [Transaction].
// pub struct RocksKVStoreImpl<'a, DB: Send + Sync>(Transaction<'a, DB>);
pub struct RocksKVStoreImpl<'a, DB: Send>(Transaction<'a, DB>);

unsafe impl<'a, DB: Send> Send for RocksKVStoreImpl<'a, DB> {}

impl<'a, DB: Send + Sync> RocksKVStoreImpl<'a, DB> {
  pub fn new(txn: Transaction<'a, DB>) -> Self {
    Self(txn)
  }

  pub fn commit_transaction(self) -> Result<(), PersistenceError> {
    self.0.commit()?;
    Ok(())
  }
}

impl<'a, DB: Send + Sync> KVStore<'a> for RocksKVStoreImpl<'a, DB> {
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
    RocksKVStoreImpl::new(txn)
  }
}

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
