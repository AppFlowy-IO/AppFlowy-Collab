use collab_entity::CollabValidateError;

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

  #[error("text_id or delta is empty")]
  TextActionParamsError,

  #[error("Lack of document required data")]
  NoRequiredData,

  #[error("The external id is not found")]
  ExternalIdIsNotFound,

  #[error("Unable to parse document to plain text")]
  ParseDocumentError,

  #[error("Unable to parse markdown to document data")]
  ParseMarkdownError,

  #[error("Unable to parse delta json to text delta")]
  ParseDeltaJsonToTextDeltaError,

  #[error("No children found")]
  NoBlockChildrenFound,

  #[error("Unknown block type: {0}")]
  UnknownBlockType(String),

  #[error("Unable to find the page block")]
  PageBlockNotFound,
}

impl From<CollabValidateError> for DocumentError {
  fn from(error: CollabValidateError) -> Self {
    match error {
      CollabValidateError::NoRequiredData(_) => DocumentError::NoRequiredData,
    }
  }
}
