#[derive(Debug, thiserror::Error)]
pub enum CollabError {
    #[error("Internal error")]
    Internal(#[from] anyhow::Error),
}
