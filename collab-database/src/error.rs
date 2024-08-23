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

  #[error("row: {row_id} not found, reason: {reason}")]
  DatabaseRowNotFound { row_id: RowId, reason: String },

  #[error("The database view is not existing")]
  DatabaseViewNotExist,

  #[error(transparent)]
  SerdeJson(#[from] serde_json::Error),

  #[error(transparent)]
  UuidError(#[from] uuid::Error),

  #[error("No required data")]
  NoRequiredData,

  #[error("Record already exist")]
  RecordAlreadyExist,

  #[error("Record not found")]
  RecordNotFound,

  #[error("Action cancelled")]
  ActionCancelled,

  #[error("Internal failure: {0}")]
  Internal(#[from] anyhow::Error),
}
