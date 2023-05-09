use crate::error::WSError;

use collab_sync::msg::CollabMessage;
use serde::{Deserialize, Serialize};
use tokio_tungstenite::tungstenite::Message;

/// The ID of the handler . Each handler can be a document, a folder, or a database.
/// The WSMessage carries the ID of the handler, so that the server can dispatch
/// the message to the corresponding target.
pub type HandlerID = String;

/// The message sent through WebSocket.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WSMessage {
  pub handler_id: HandlerID,
  pub payload: Vec<u8>,
}

impl WSMessage {
  pub fn new(handler_id: HandlerID, payload: Vec<u8>) -> Self {
    Self {
      handler_id,
      payload,
    }
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

impl From<CollabMessage> for WSMessage {
  fn from(msg: CollabMessage) -> Self {
    let handler_id = msg.object_id().to_string();
    let payload = msg.to_vec();
    Self {
      handler_id,
      payload,
    }
  }
}

impl TryFrom<WSMessage> for CollabMessage {
  type Error = WSError;

  fn try_from(value: WSMessage) -> Result<Self, Self::Error> {
    let msg = CollabMessage::from_vec(value.payload).map_err(|e| WSError::Internal(Box::new(e)))?;
    Ok(msg)
  }
}
