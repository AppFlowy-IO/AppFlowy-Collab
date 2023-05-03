#[derive(Debug, thiserror::Error)]
pub enum CollabError {
  #[error(transparent)]
  Persistence(#[from] collab_persistence::error::PersistenceError),

  #[error(transparent)]
  SerdeJson(#[from] serde_json::Error),

  #[error("Internal failure: {0}")]
  Internal(#[from] Box<dyn std::error::Error + Send + Sync>),
}
