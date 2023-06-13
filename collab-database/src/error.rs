#[derive(Debug, thiserror::Error)]
pub enum DatabaseError {
  #[error("The database's id is invalid: {0}")]
  InvalidDatabaseID(&'static str),

  #[error("The database view's id is invalid: {0}")]
  InvalidViewID(&'static str),

  #[error("The database row's id is invalid: {0}")]
  InvalidRowID(&'static str),

  #[error("The database is not existing")]
  DatabaseNotExist,

  #[error("The database view is not existing")]
  DatabaseViewNotExist,

  #[error("Can not decode the data to update")]
  DecodeUpdate(#[from] collab::preclude::lib0Error),

  #[error(transparent)]
  SerdeJson(#[from] serde_json::Error),

  #[error(transparent)]
  UuidError(#[from] uuid::Error),

  #[error("Internal error")]
  Internal(#[from] anyhow::Error),
}
