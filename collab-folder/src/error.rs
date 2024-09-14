use collab_entity::CollabValidateError;

#[derive(Debug, thiserror::Error)]
pub enum FolderError {
  #[error(transparent)]
  Internal(#[from] anyhow::Error),

  #[error(transparent)]
  CollabError(#[from] collab::error::CollabError),

  #[error("Lack of folder required data:{0}")]
  NoRequiredData(String),
}

impl From<CollabValidateError> for FolderError {
  fn from(error: CollabValidateError) -> Self {
    match error {
      CollabValidateError::NoRequiredData(data) => FolderError::NoRequiredData(data),
    }
  }
}
