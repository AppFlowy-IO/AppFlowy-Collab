use crate::error::SyncError;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum CollabMessage {
  Server(CollabServerMessage),
  Ack(CollabAckMessage),
  Client(CollabClientMessage),
}

impl Display for CollabMessage {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    match self {
      CollabMessage::Server(value) => f.write_fmt(format_args!(
        "Server|oid:{}|payload_len:{}|",
        value.object_id,
        value.payload.len(),
      )),
      CollabMessage::Client(value) => f.write_fmt(format_args!(
        "Client|uid:{}|oid:{}|payload_len:{}|msg_id:{}|",
        value.from_uid,
        value.object_id,
        value.payload.len(),
        value.msg_id,
      )),
      CollabMessage::Ack(value) => f.write_fmt(format_args!(
        "Ack|oid:{}|msg_id:{}|",
        value.object_id, value.msg_id,
      )),
    }
  }
}

impl CollabMessage {
  pub fn to_vec(&self) -> Vec<u8> {
    serde_json::to_vec(self).unwrap_or_default()
  }

  pub fn from_vec(data: Vec<u8>) -> Result<Self, SyncError> {
    serde_json::from_slice(&data).map_err(SyncError::SerdeError)
  }

  pub fn into_payload(self) -> Vec<u8> {
    match self {
      CollabMessage::Server(value) => value.payload,
      CollabMessage::Client(value) => value.payload,
      CollabMessage::Ack(_) => vec![],
    }
  }

  pub fn payload(&self) -> Option<&Vec<u8>> {
    match self {
      CollabMessage::Server(value) => Some(&value.payload),
      CollabMessage::Client(value) => Some(&value.payload),
      CollabMessage::Ack(_) => None,
    }
  }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CollabServerMessage {
  object_id: String,
  payload: Vec<u8>,
}

impl CollabServerMessage {
  pub fn new(object_id: String, payload: Vec<u8>) -> Self {
    Self { object_id, payload }
  }
}

impl From<CollabServerMessage> for CollabMessage {
  fn from(value: CollabServerMessage) -> Self {
    CollabMessage::Server(value)
  }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CollabClientMessage {
  from_uid: i64,
  object_id: String,
  msg_id: u32,
  payload: Vec<u8>,
}

impl CollabClientMessage {
  pub fn new(from_uid: i64, object_id: String, msg_id: u32, payload: Vec<u8>) -> Self {
    Self {
      from_uid,
      object_id,
      msg_id,
      payload,
    }
  }
}

impl From<CollabClientMessage> for CollabMessage {
  fn from(value: CollabClientMessage) -> Self {
    CollabMessage::Client(value)
  }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CollabAckMessage {
  object_id: String,
  msg_id: u32,
}

impl From<CollabAckMessage> for CollabMessage {
  fn from(value: CollabAckMessage) -> Self {
    CollabMessage::Ack(value)
  }
}
