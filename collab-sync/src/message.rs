use crate::error::SyncError;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum CollabMessage {
  Init(CollabInitMessage),
  Server(CollabServerMessage),
  Ack(CollabAckMessage),
  Client(CollabClientMessage),
}

impl CollabMessage {
  pub fn is_init(&self) -> bool {
    matches!(self, CollabMessage::Init(_))
  }

  pub fn msg_id(&self) -> Option<u32> {
    match self {
      CollabMessage::Init(value) => Some(value.msg_id),
      CollabMessage::Server(_) => None,
      CollabMessage::Client(value) => Some(value.msg_id),
      CollabMessage::Ack(value) => Some(value.msg_id),
    }
  }

  pub fn is_empty(&self) -> bool {
    match self {
      CollabMessage::Init(value) => value.payload.is_empty(),
      CollabMessage::Server(value) => value.payload.is_empty(),
      CollabMessage::Client(value) => value.payload.is_empty(),
      CollabMessage::Ack(_) => true,
    }
  }
}

impl Display for CollabMessage {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    match self {
      CollabMessage::Init(value) => f.write_fmt(format_args!(
        "Init|uid:{}|oid:{}|payload_len:{}|msg_id:{}|",
        value.from_uid,
        value.object_id,
        value.payload.len(),
        value.msg_id,
      )),
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
      CollabMessage::Init(value) => value.payload,
      CollabMessage::Server(value) => value.payload,
      CollabMessage::Client(value) => value.payload,
      CollabMessage::Ack(_) => vec![],
    }
  }

  pub fn payload(&self) -> Option<&Vec<u8>> {
    match self {
      CollabMessage::Init(value) => Some(&value.payload),
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
  pub object_id: String,
  pub msg_id: u32,
}

impl CollabAckMessage {
  pub fn new(object_id: String, msg_id: u32) -> Self {
    Self { object_id, msg_id }
  }
}

impl From<CollabAckMessage> for CollabMessage {
  fn from(value: CollabAckMessage) -> Self {
    CollabMessage::Ack(value)
  }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CollabInitMessage {
  pub from_uid: i64,
  pub object_id: String,
  pub msg_id: u32,
  pub payload: Vec<u8>,
  pub md5: String,
}

impl CollabInitMessage {
  pub fn new(from_uid: i64, object_id: String, msg_id: u32, payload: Vec<u8>) -> Self {
    let md5 = md5(&payload);
    Self {
      from_uid,
      object_id,
      msg_id,
      payload,
      md5,
    }
  }
}

impl From<CollabInitMessage> for CollabMessage {
  fn from(value: CollabInitMessage) -> Self {
    CollabMessage::Init(value)
  }
}

pub fn md5<T: AsRef<[u8]>>(data: T) -> String {
  let digest = md5::compute(data);
  format!("{:x}", digest)
}
