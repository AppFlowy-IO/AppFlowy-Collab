use collab::sync_protocol;

#[derive(Debug, thiserror::Error)]
pub enum SyncError {
  #[error(transparent)]
  YSync(#[from] sync_protocol::message::Error),

  #[error(transparent)]
  YAwareness(#[from] sync_protocol::awareness::Error),

  #[error("failed to deserialize message: {0}")]
  DecodingError(#[from] yrs::encoding::read::Error),

  #[error(transparent)]
  SerdeError(#[from] serde_json::Error),

  #[error(transparent)]
  TokioTask(#[from] tokio::task::JoinError),

  #[error(transparent)]
  IO(#[from] std::io::Error),

  #[error("Internal failure: {0}")]
  Internal(#[from] Box<dyn std::error::Error + Send + Sync>),
}
