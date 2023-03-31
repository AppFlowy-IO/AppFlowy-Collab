#[derive(Debug, thiserror::Error)]
pub enum DatabaseError {
  #[error("The database's id is invalid")]
  InvalidDatabaseID,

  #[error("Internal error")]
  Internal(#[from] anyhow::Error),
}
