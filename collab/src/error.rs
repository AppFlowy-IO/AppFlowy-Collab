#[derive(Debug, thiserror::Error)]
pub enum CollabError {
    #[error(transparent)]
    Persistence(#[from] collab_persistence::error::PersistenceError),

    #[error("Internal error")]
    Internal(#[from] anyhow::Error),
}
