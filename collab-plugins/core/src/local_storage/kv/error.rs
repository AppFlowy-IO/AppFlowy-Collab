#[derive(Debug, thiserror::Error)]
pub enum PersistenceError {
  #[cfg(feature = "rocksdb_plugin")]
  #[error("Rocksdb corruption:{0}")]
  RocksdbCorruption(String),

  #[cfg(feature = "rocksdb_plugin")]
  #[error("Rocksdb repair:{0}")]
  RocksdbRepairFail(String),

  #[cfg(feature = "rocksdb_plugin")]
  #[error("{0}")]
  RocksdbBusy(String),

  // If the database is already locked by another process, it will return an IO error. It
  // happens when the database is already opened by another process.
  #[cfg(feature = "rocksdb_plugin")]
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
  Yrs(#[from] yrs::encoding::read::Error),

  #[error("invalid data: {0}")]
  InvalidData(String),

  #[error("Duplicate update key")]
  DuplicateUpdateKey,

  #[error("Can't find the latest update key")]
  LatestUpdateKeyNotExist,

  #[error(transparent)]
  Internal(#[from] anyhow::Error),
}

#[cfg(feature = "rocksdb_plugin")]
impl From<rocksdb::Error> for PersistenceError {
  fn from(value: rocksdb::Error) -> Self {
    match value.kind() {
      rocksdb::ErrorKind::NotFound => PersistenceError::UnexpectedEmptyUpdates,
      rocksdb::ErrorKind::Corruption => PersistenceError::RocksdbCorruption(value.into_string()),
      rocksdb::ErrorKind::IOError => PersistenceError::RocksdbIOError(value.into_string()),
      rocksdb::ErrorKind::Busy => PersistenceError::RocksdbBusy(value.into_string()),
      _ => PersistenceError::Internal(value.into()),
    }
  }
}
