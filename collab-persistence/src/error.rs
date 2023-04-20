#[derive(Debug, thiserror::Error)]
pub enum PersistenceError {
  #[error(transparent)]
  SledDb(#[from] sled::Error),

  #[error(transparent)]
  RocksDb(#[from] rocksdb::Error),

  #[error(transparent)]
  Bincode(#[from] bincode::Error),

  #[error("The document is not exist")]
  DocumentNotExist,

  #[error(transparent)]
  Yrs(#[from] lib0::error::Error),

  #[error("invalid data")]
  InvalidData,

  #[error("Internal error")]
  InternalError,
}
