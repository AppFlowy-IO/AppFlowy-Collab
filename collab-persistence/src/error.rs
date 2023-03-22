#[derive(Debug, thiserror::Error)]
pub enum PersistenceError {
    #[error(transparent)]
    Db(#[from] sled::Error),

    #[error("Serde error")]
    SerdeError,

    #[error("The document is not exist")]
    DocumentNotExist,

    #[error(transparent)]
    Yrs(#[from] lib0::error::Error),

    #[error("invalid data")]
    InvalidData,
}
