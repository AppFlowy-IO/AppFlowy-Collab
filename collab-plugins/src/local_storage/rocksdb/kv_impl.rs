use std::ops;
use std::ops::RangeBounds;
use std::path::Path;
use std::sync::Arc;

use crate::local_storage::kv::doc::CollabKVAction;

use crate::local_storage::kv::{KVEntry, KVStore, KVTransactionDB, PersistenceError};
use rocksdb::Direction::Forward;
use rocksdb::{
  DBIteratorWithThreadMode, Direction, ErrorKind, IteratorMode, Options, ReadOptions,
  SingleThreaded, Transaction, TransactionDB, TransactionDBOptions, TransactionOptions,
  WriteOptions,
};

#[derive(Clone)]
pub struct KVTransactionDBRocksdbImpl {
  db: Arc<TransactionDB>,
}

impl KVTransactionDBRocksdbImpl {
  /// Open a new RocksDB database at the given path.
  /// If the database is corrupted, try to repair it. If it cannot be repaired, return an error.
  pub fn open(path: impl AsRef<Path>) -> Result<Self, PersistenceError> {
    let auto_repair = false;
    let txn_db_opts = TransactionDBOptions::default();
    let mut db_opts = Options::default();
    // This option sets the upper limit for the total number of background jobs (both flushes and compactions)
    // that can run concurrently. If you set this value too low, you might limit the ability of RocksDB to
    // efficiently flush and compact data, potentially leading to increased write latency or larger disk space usage.
    // On the other hand, setting it too high could lead to excessive CPU and I/O usage, impacting the overall
    // performance of the system.
    db_opts.set_max_background_jobs(4);
    db_opts.create_if_missing(true);

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
    // don't set the log dir (set_db_log_dir) because it will cause the 'file name too long' error on mobile platform
    db_opts.set_recycle_log_file_num(5);
    db_opts.set_keep_log_file_num(5);

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

  pub async fn is_exist(
    &self,
    uid: i64,
    workspace_id: &str,
    object_id: &str,
  ) -> Result<bool, PersistenceError> {
    let read_txn = self.read_txn();
    Ok(read_txn.is_exist(uid, workspace_id, object_id))
  }

  pub async fn delete_doc(
    &self,
    uid: i64,
    workspace_id: &str,
    doc_id: &str,
  ) -> Result<(), PersistenceError> {
    self.with_write_txn(|txn| txn.delete_doc(uid, workspace_id, doc_id))?;
    Ok(())
  }
}

impl KVTransactionDB for KVTransactionDBRocksdbImpl {
  type TransactionAction<'a> = RocksdbKVStoreImpl<'a, TransactionDB>;

  fn read_txn<'a, 'b>(&'b self) -> Self::TransactionAction<'a>
  where
    'b: 'a,
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
    RocksdbKVStoreImpl::new(txn)
  }

  fn write_txn<'a, 'b>(&'b self) -> Self::TransactionAction<'a>
  where
    'b: 'a,
  {
    let txn_options = TransactionOptions::default();
    let txn = self
      .db
      .transaction_opt(&WriteOptions::default(), &txn_options);
    RocksdbKVStoreImpl::new(txn)
  }

  fn with_write_txn<'a, 'b, Output>(
    &'b self,
    f: impl FnOnce(&Self::TransactionAction<'a>) -> Result<Output, PersistenceError>,
  ) -> Result<Output, PersistenceError>
  where
    'b: 'a,
  {
    let txn_options = TransactionOptions::default();
    let txn = self
      .db
      .transaction_opt(&WriteOptions::default(), &txn_options);
    let store = RocksdbKVStoreImpl::new(txn);
    let result = f(&store)?;
    store.0.commit()?;
    Ok(result)
  }

  fn flush(&self) -> Result<(), PersistenceError> {
    Ok(())
  }
}

/// Implementation of [KVStore] for [KVTransactionDBRocksdbImpl]. This is a wrapper around [Transaction].
// pub struct RocksKVStoreImpl<'a, DB: Send + Sync>(Transaction<'a, DB>);
pub struct RocksdbKVStoreImpl<'a, DB: Send>(Transaction<'a, DB>);

unsafe impl<DB: Send> Send for RocksdbKVStoreImpl<'_, DB> {}

impl<'a, DB: Send + Sync> RocksdbKVStoreImpl<'a, DB> {
  pub fn new(txn: Transaction<'a, DB>) -> Self {
    Self(txn)
  }

  pub fn commit_transaction(self) -> Result<(), PersistenceError> {
    self.0.commit()?;
    Ok(())
  }
}

impl<'a, DB: Send + Sync> KVStore<'a> for RocksdbKVStoreImpl<'a, DB> {
  type Range = RocksdbRange<'a, DB>;
  type Entry = RocksdbEntry;
  type Value = Vec<u8>;
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
    Ok(RocksdbRange {
      // Safe to transmute because the lifetime of the iterator is the same as the lifetime of the
      // transaction.
      inner: unsafe {
        std::mem::transmute::<
          rocksdb::DBIteratorWithThreadMode<'_, rocksdb::Transaction<'_, DB>>,
          rocksdb::DBIteratorWithThreadMode<'_, rocksdb::Transaction<'_, DB>>,
        >(iter)
      },
      to: to.to_vec(),
    })
  }

  fn next_back_entry(&self, key: &[u8]) -> Result<Option<Self::Entry>, Self::Error> {
    let opt = ReadOptions::default();
    let mut raw = self.0.raw_iterator_opt(opt);
    raw.seek_for_prev(key);
    if let Some((key, value)) = raw.item() {
      Ok(Some(RocksdbEntry::new(key.to_vec(), value.to_vec())))
    } else {
      Ok(None)
    }
  }
}

impl<'a, DB: Send + Sync> From<Transaction<'a, DB>> for RocksdbKVStoreImpl<'a, DB> {
  #[inline(always)]
  fn from(txn: Transaction<'a, DB>) -> Self {
    RocksdbKVStoreImpl::new(txn)
  }
}

pub struct RocksdbRange<'a, DB> {
  inner: DBIteratorWithThreadMode<'a, Transaction<'a, DB>>,
  to: Vec<u8>,
}

impl<DB: Send + Sync> Iterator for RocksdbRange<'_, DB> {
  type Item = RocksdbEntry;

  fn next(&mut self) -> Option<Self::Item> {
    let n = self.inner.next()?;
    if let Ok((key, value)) = n {
      if key.as_ref() >= self.to.as_slice() {
        None
      } else {
        Some(RocksdbEntry::new(key.to_vec(), value.to_vec()))
      }
    } else {
      None
    }
  }
}

pub struct RocksdbEntry {
  key: Vec<u8>,
  value: Vec<u8>,
}

impl RocksdbEntry {
  pub fn new(key: Vec<u8>, value: Vec<u8>) -> Self {
    Self { key, value }
  }
}

impl KVEntry for RocksdbEntry {
  fn key(&self) -> &[u8] {
    self.key.as_ref()
  }

  fn value(&self) -> &[u8] {
    self.value.as_ref()
  }
}
