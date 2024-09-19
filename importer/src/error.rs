use std::str::Utf8Error;

#[derive(Debug, thiserror::Error)]
pub enum ImporterError {
  #[error("Invalid path: {0}")]
  InvalidPath(String),

  #[error("Invalid path format")]
  InvalidPathFormat,

  #[error("{0}")]
  InvalidFileType(String),

  #[error(transparent)]
  ImportMarkdownError(#[from] collab_document::error::DocumentError),

  #[error(transparent)]
  ImportCsvError(#[from] collab_database::error::DatabaseError),

  #[error("Parse markdown error: {0}")]
  ParseMarkdownError(markdown::message::Message),

  #[error(transparent)]
  Utf8Error(#[from] Utf8Error),

  #[error(transparent)]
  Internal(#[from] anyhow::Error),
}

impl From<std::io::Error> for ImporterError {
  fn from(error: std::io::Error) -> Self {
    Self::Internal(error.into())
  }
}
