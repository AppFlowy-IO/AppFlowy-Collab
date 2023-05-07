use crate::error::WSError;

use serde::{Deserialize, Serialize};
use tokio_tungstenite::tungstenite::Message;

/// The ID of a target. Each target can be a document, a folder, or a database.
/// The WSMessage carries the ID of the target, so that the server can dispatch
/// the message to the corresponding target.
pub type TargetID = String;

/// The message sent through WebSocket.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WSMessage {
  pub id: TargetID,
  pub payload: Vec<u8>,
}

impl WSMessage {
  pub fn new(id: TargetID, payload: Vec<u8>) -> Self {
    Self { id, payload }
  }
}

impl TryFrom<&Message> for WSMessage {
  type Error = WSError;

  fn try_from(value: &Message) -> Result<Self, Self::Error> {
    match value {
      Message::Binary(bytes) => {
        let msg = serde_json::from_slice::<WSMessage>(bytes)?;
        Ok(msg)
      },
      _ => Err(WSError::UnsupportedMsgType),
    }
  }
}

impl From<WSMessage> for Message {
  fn from(msg: WSMessage) -> Self {
    let bytes = serde_json::to_vec(&msg).unwrap_or_default();
    Message::Binary(bytes)
  }
}
