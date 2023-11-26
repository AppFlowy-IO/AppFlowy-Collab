#[derive(Debug, thiserror::Error)]
pub enum PersistenceError {
  #[cfg(feature = "sled_db_persistence")]
  #[error(transparent)]
  SledDb(#[from] sled::Error),

  #[cfg(feature = "rocksdb_persistence")]
  #[error(transparent)]
  RocksDb(#[from] rocksdb::Error),

  #[cfg(feature = "rocksdb_persistence")]
  #[error("Rocksdb corruption:{0}")]
  RocksdbCorruption(String),

  #[cfg(feature = "rocksdb_persistence")]
  #[error("Rocksdb repair:{0}")]
  RocksdbRepairFail(String),

  #[cfg(feature = "rocksdb_persistence")]
  #[error("{0}")]
  RocksdbBusy(String),

  // If the database is already locked by another process, it will return an IO error. It
  // happens when the database is already opened by another process.
  #[cfg(feature = "rocksdb_persistence")]
  #[error("{0}")]
  RocksdbIOError(String),

  #[error(transparent)]
  Bincode(#[from] bincode::Error),

  #[error("The document is not exist")]
  DocumentNotExist,

  #[error("The document already exist")]
  DocumentAlreadyExist,

  #[error("Unexpected empty updates")]
  UnexpectedEmptyUpdates,

  #[error(transparent)]
  Yrs(#[from] lib0::error::Error),

  #[error("invalid data: {0}")]
  InvalidData(String),

  #[error("Duplicate update key")]
  DuplicateUpdateKey,

  #[error("Can't find the latest update key")]
  LatestUpdateKeyNotExist,

  #[error(transparent)]
  Internal(#[from] anyhow::Error),
}
