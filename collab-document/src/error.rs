#[derive(Debug, thiserror::Error)]
pub enum DocumentError {
  #[error(transparent)]
  Internal(#[from] anyhow::Error),

  #[error("Could not create block")]
  BlockCreateError,

  #[error("The block is existed already")]
  BlockIsExistedAlready,

  #[error("The block is not found")]
  BlockIsNotFound,

  #[error("Could not convert json to data")]
  ConvertDataError,

  #[error("The parent is not found")]
  ParentIsNotFound,

  #[error("Could not create root block")]
  CreateRootBlockError,

  #[error("Could not delete block")]
  DeleteBlockError,
}
