#[derive(Debug, thiserror::Error)]
pub enum DocumentError {
  #[error(transparent)]
  Internal(#[from] anyhow::Error),

  #[error("The block is create failed")]
  BlockCreateError { block_id: String },
}
