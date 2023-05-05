#[derive(Debug, thiserror::Error)]
pub enum WSError {
  #[error(transparent)]
  Tungstenite(#[from] tokio_tungstenite::tungstenite::error::Error),

  #[error("Internal failure: {0}")]
  Internal(#[from] Box<dyn std::error::Error + Send + Sync>),
}
