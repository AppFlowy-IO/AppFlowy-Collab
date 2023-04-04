#[derive(Debug, thiserror::Error)]
pub enum DocumentError {
  #[error(transparent)]
  Internal(#[from] anyhow::Error),

  #[error("The block is existed already")]
  BlockIsExistedAlready,

  #[error("The block is not found")]
  BlockIsNotFound,

  #[error("Could not convert json to data")]
  ConvertDataError,
}
