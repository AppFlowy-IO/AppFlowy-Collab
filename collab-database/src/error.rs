use crate::rows::RowId;

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

  #[error("The database row with id {0} doesn't exist")]
  DatabaseRowNotExist(RowId),

  #[error("The database view is not existing")]
  DatabaseViewNotExist,

  #[error(transparent)]
  SerdeJson(#[from] serde_json::Error),

  #[error(transparent)]
  UuidError(#[from] uuid::Error),

  #[error("No required data")]
  NoRequiredData,

  #[error("Internal failure: {0}")]
  Internal(#[from] anyhow::Error),
}
