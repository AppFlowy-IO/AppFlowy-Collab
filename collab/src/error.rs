use anyhow::anyhow;
use yrs::TransactionAcqError;

#[derive(Debug, thiserror::Error)]
pub enum CollabError {
  #[error(transparent)]
  SerdeJson(#[from] serde_json::Error),

  #[error("Unexpected empty: {0}")]
  UnexpectedEmpty(String),

  #[error("Get write txn failed")]
  AcquiredWriteTxnFail,

  #[error("Get read txn failed")]
  AcquiredReadTxnFail,

  #[error("Try apply update failed: {0}")]
  YrsTransactionError(String),

  #[error("Try encode update failed: {0}")]
  YrsEncodeStateError(String),

  #[error("UndoManager is not enabled")]
  UndoManagerNotEnabled,

  #[error(transparent)]
  DecodeUpdate(#[from] yrs::encoding::read::Error),

  #[error("{0}")]
  NoRequiredData(String),

  #[error(transparent)]
  Awareness(#[from] crate::core::awareness::Error),

  #[error("Failed to apply update: {0}")]
  UpdateFailed(#[from] yrs::error::UpdateError),

  #[error("Internal failure: {0}")]
  Internal(#[from] anyhow::Error),
}

impl From<TransactionAcqError> for CollabError {
  fn from(value: TransactionAcqError) -> Self {
    match value {
      TransactionAcqError::SharedAcqFailed => Self::AcquiredReadTxnFail,
      TransactionAcqError::ExclusiveAcqFailed => Self::AcquiredWriteTxnFail,
      TransactionAcqError::DocumentDropped => Self::Internal(anyhow!("Document dropped")),
    }
  }
}
