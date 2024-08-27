#[derive(Debug, thiserror::Error)]
pub enum PersistenceError {
  #[cfg(not(target_arch = "wasm32"))]
  #[error("Rocksdb corruption:{0}")]
  RocksdbCorruption(String),

  #[cfg(not(target_arch = "wasm32"))]
  #[error("Rocksdb repair:{0}")]
  RocksdbRepairFail(String),

  #[cfg(not(target_arch = "wasm32"))]
  #[error("{0}")]
  RocksdbBusy(String),

  // If the database is already locked by another process, it will return an IO error. It
  // happens when the database is already opened by another process.
  #[cfg(not(target_arch = "wasm32"))]
  #[error("{0}")]
  RocksdbIOError(String),

  #[error(transparent)]
  Bincode(#[from] bincode::Error),

  #[error("{0}")]
  RecordNotFound(String),

  #[error("The document already exist")]
  DocumentAlreadyExist,

  #[error("Unexpected empty updates")]
  UnexpectedEmptyUpdates,

  #[error(transparent)]
  Yrs(#[from] yrs::encoding::read::Error),

  #[error("Failed to apply update from persistent store: {0}")]
  Update(#[from] yrs::error::UpdateError),

  #[error("invalid data: {0}")]
  InvalidData(String),

  #[error("Duplicate update key")]
  DuplicateUpdateKey,

  #[error("Can't find the latest update key")]
  LatestUpdateKeyNotExist,

  #[error(transparent)]
  Collab(#[from] collab::error::CollabError),

  #[error(transparent)]
  Internal(#[from] anyhow::Error),
}

impl PersistenceError {
  pub fn is_record_not_found(&self) -> bool {
    matches!(self, PersistenceError::RecordNotFound(_))
  }
}

#[cfg(target_arch = "wasm32")]
impl From<indexed_db_futures::web_sys::DomException> for PersistenceError {
  fn from(value: indexed_db_futures::web_sys::DomException) -> Self {
    PersistenceError::Internal(anyhow::anyhow!("DOMException: {:?}", value))
  }
}

#[cfg(not(target_arch = "wasm32"))]
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
