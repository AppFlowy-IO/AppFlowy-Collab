use crate::rows::RowId;
use collab_entity::CollabValidateError;

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

  #[error("No required data:{0}")]
  NoRequiredData(String),

  #[error("Record already exist")]
  RecordAlreadyExist,

  #[error("Record not found")]
  RecordNotFound,

  #[error("Action cancelled")]
  ActionCancelled,

  #[error("Invalid CSV:{0}")]
  InvalidCSV(String),

  #[error("Import data failed: {0}")]
  ImportData(String),

  #[error("Internal failure: {0}")]
  Internal(#[from] anyhow::Error),
}

impl DatabaseError {
  pub fn is_no_required_data(&self) -> bool {
    matches!(self, DatabaseError::NoRequiredData(_))
  }
}

impl From<CollabValidateError> for DatabaseError {
  fn from(error: CollabValidateError) -> Self {
    match error {
      CollabValidateError::NoRequiredData(data) => DatabaseError::NoRequiredData(data),
    }
  }
}
