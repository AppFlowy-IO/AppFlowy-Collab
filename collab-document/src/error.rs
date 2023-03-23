#[derive(Debug, thiserror::Error)]
pub enum DocumentError {
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}
