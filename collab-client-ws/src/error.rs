use crate::msg::WSMessage;
use tokio_stream::wrappers::errors::BroadcastStreamRecvError;

#[derive(Debug, thiserror::Error)]
pub enum WSError {
  #[error(transparent)]
  Tungstenite(#[from] tokio_tungstenite::tungstenite::error::Error),

  #[error("Unsupported ws message type")]
  UnsupportedMsgType,

  #[error(transparent)]
  SerdeError(#[from] serde_json::Error),

  #[error(transparent)]
  SenderError(#[from] tokio::sync::broadcast::error::SendError<WSMessage>),

  #[error(transparent)]
  BroadcastStreamRecvError(#[from] BroadcastStreamRecvError),

  #[error("Internal failure: {0}")]
  Internal(#[from] Box<dyn std::error::Error + Send + Sync>),
}
