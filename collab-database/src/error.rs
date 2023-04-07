#[derive(Debug, thiserror::Error)]
pub enum DatabaseError {
  #[error("The database's id is invalid")]
  InvalidDatabaseID,

  #[error("The database is not existing")]
  DatabaseNotExist,

  #[error("The database view is not existing")]
  DatabaseViewNotExist,

  #[error("Can not decode the data to update")]
  DecodeUpdate(#[from] collab::preclude::lib0Error),

  #[error("Internal error")]
  Internal(#[from] anyhow::Error),
}
