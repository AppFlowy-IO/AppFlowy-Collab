#[derive(Debug, thiserror::Error)]
pub enum PersistenceError {
  #[cfg(feature = "sled_db")]
  #[error(transparent)]
  SledDb(#[from] sled::Error),

  #[cfg(feature = "rocksdb_db")]
  #[error(transparent)]
  RocksDb(#[from] rocksdb::Error),

  #[error(transparent)]
  Bincode(#[from] bincode::Error),

  #[error("The document is not exist")]
  DocumentNotExist,

  #[error("The document already exist")]
  DocumentAlreadyExist,

  #[error(transparent)]
  Yrs(#[from] lib0::error::Error),

  #[error("invalid data: {0}")]
  InvalidData(String),

  #[error("Duplicate update key")]
  DuplicateUpdateKey,

  #[error("Can't find the latest update key")]
  LatestUpdateKeyNotExist,

  #[error("Internal failure: {0}")]
  Internal(#[from] Box<dyn std::error::Error + Send + Sync>),
}
