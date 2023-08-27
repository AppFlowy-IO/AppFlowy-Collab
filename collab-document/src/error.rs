#[derive(Debug, thiserror::Error)]
pub enum DocumentError {
  #[error(transparent)]
  Internal(#[from] anyhow::Error),

  #[error(transparent)]
  CollabError(#[from] collab::error::CollabError),

  #[error("Could not create block")]
  BlockCreateError,

  #[error("The block already exists")]
  BlockAlreadyExists,

  #[error("The block is not found")]
  BlockIsNotFound,

  #[error("The page id empty")]
  PageIdIsEmpty,

  #[error("Could not convert json to data")]
  ConvertDataError,

  #[error("The parent is not found")]
  ParentIsNotFound,

  #[error("Could not create the root block due to an unspecified error")]
  CreateRootBlockError,

  #[error("Could not delete block")]
  DeleteBlockError,
}
