#[derive(Debug, thiserror::Error)]
pub enum CollabError {
  #[error(transparent)]
  SerdeJson(#[from] serde_json::Error),

  #[error("Unexpected empty value")]
  UnexpectedEmpty,

  #[error("Internal failure: {0}")]
  Internal(#[from] Box<dyn std::error::Error + Send + Sync>),
}
