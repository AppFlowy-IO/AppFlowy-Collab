use std::fmt::{Display, Formatter};

use collab::core::collab::CollabOrigin;
use serde::{Deserialize, Serialize};

use crate::error::SyncError;

#[derive(Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub enum CollabMessage {
  ClientInit(ClientInitMessage),
  ClientUpdate(ClientUpdateMessage),
  AwarenessUpdate(AwarenessUpdateMessage),
  BroadcastUpdate(BroadcastUpdateMessage),
  ServerAck(CollabAckMessage),
}

impl CollabMessage {
  pub fn is_init(&self) -> bool {
    matches!(self, CollabMessage::ClientInit(_))
  }

  pub fn msg_id(&self) -> Option<u32> {
    match self {
      CollabMessage::ClientInit(value) => Some(value.msg_id),
      CollabMessage::ClientUpdate(value) => Some(value.msg_id),
      CollabMessage::BroadcastUpdate(_) => None,
      CollabMessage::AwarenessUpdate(_) => None,
      CollabMessage::ServerAck(value) => Some(value.msg_id),
    }
  }

  pub fn is_empty(&self) -> bool {
    match self {
      CollabMessage::ClientInit(value) => value.payload.is_empty(),
      CollabMessage::ClientUpdate(value) => value.payload.is_empty(),
      CollabMessage::BroadcastUpdate(value) => value.payload.is_empty(),
      CollabMessage::AwarenessUpdate(value) => value.payload.is_empty(),
      CollabMessage::ServerAck(_) => true,
    }
  }

  pub fn origin(&self) -> CollabOrigin {
    match self {
      CollabMessage::ClientInit(value) => value.origin.clone(),
      CollabMessage::ClientUpdate(value) => value.origin.clone(),
      CollabMessage::BroadcastUpdate(value) => value.origin.clone(),
      CollabMessage::AwarenessUpdate(_) => CollabOrigin::default(),
      CollabMessage::ServerAck(_) => CollabOrigin::default(),
    }
  }
}

impl Display for CollabMessage {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    match self {
      CollabMessage::ClientInit(value) => f.write_fmt(format_args!(
        "ClientInit: [uid:{}|device_id:{}|oid:{}|payload_len:{}|msg_id:{}]",
        value.origin.uid,
        value.origin.device_id,
        value.object_id,
        value.payload.len(),
        value.msg_id,
      )),
      CollabMessage::ClientUpdate(value) => f.write_fmt(format_args!(
        "ClientUpdate: [uid:{}|device_id:{}|oid:{}|payload_len:{}|msg_id:{}]",
        value.origin.uid,
        value.origin.device_id,
        value.object_id,
        value.payload.len(),
        value.msg_id,
      )),
      CollabMessage::BroadcastUpdate(value) => f.write_fmt(format_args!(
        "BroadcastUpdate: [uid:{}|device_id:{}|oid:{}|payload_len:{}]",
        value.origin.uid,
        value.origin.device_id,
        value.object_id,
        value.payload.len(),
      )),
      CollabMessage::AwarenessUpdate(value) => f.write_fmt(format_args!(
        "Awareness: [oid:{}|payload_len:{}]",
        value.object_id,
        value.payload.len(),
      )),
      CollabMessage::ServerAck(value) => f.write_fmt(format_args!(
        "Ack: [oid:{}|msg_id:{}]",
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
      CollabMessage::ClientInit(value) => value.payload,
      CollabMessage::ClientUpdate(value) => value.payload,
      CollabMessage::BroadcastUpdate(value) => value.payload,
      CollabMessage::AwarenessUpdate(value) => value.payload,
      CollabMessage::ServerAck(_) => Vec::new(),
    }
  }

  pub fn payload(&self) -> Option<&Vec<u8>> {
    match self {
      CollabMessage::ClientInit(value) => Some(&value.payload),
      CollabMessage::ClientUpdate(value) => Some(&value.payload),
      CollabMessage::BroadcastUpdate(value) => Some(&value.payload),
      CollabMessage::AwarenessUpdate(value) => Some(&value.payload),
      CollabMessage::ServerAck(_) => None,
    }
  }
}

#[derive(Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub struct AwarenessUpdateMessage {
  object_id: String,
  payload: Vec<u8>,
}

impl AwarenessUpdateMessage {
  pub fn new(object_id: String, payload: Vec<u8>) -> Self {
    Self { object_id, payload }
  }
}

impl From<AwarenessUpdateMessage> for CollabMessage {
  fn from(value: AwarenessUpdateMessage) -> Self {
    CollabMessage::AwarenessUpdate(value)
  }
}

#[derive(Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub struct ClientUpdateMessage {
  origin: CollabOrigin,
  object_id: String,
  msg_id: u32,
  payload: Vec<u8>,
}

impl ClientUpdateMessage {
  pub fn new(origin: CollabOrigin, object_id: String, msg_id: u32, payload: Vec<u8>) -> Self {
    Self {
      origin,
      object_id,
      msg_id,
      payload,
    }
  }
}

impl From<ClientUpdateMessage> for CollabMessage {
  fn from(value: ClientUpdateMessage) -> Self {
    CollabMessage::ClientUpdate(value)
  }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
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
    CollabMessage::ServerAck(value)
  }
}

#[derive(Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub struct ClientInitMessage {
  pub origin: CollabOrigin,
  pub object_id: String,
  pub msg_id: u32,
  pub payload: Vec<u8>,
  pub md5: String,
}

impl ClientInitMessage {
  pub fn new(origin: CollabOrigin, object_id: String, msg_id: u32, payload: Vec<u8>) -> Self {
    let md5 = md5(&payload);
    Self {
      origin,
      object_id,
      msg_id,
      payload,
      md5,
    }
  }
}

impl From<ClientInitMessage> for CollabMessage {
  fn from(value: ClientInitMessage) -> Self {
    CollabMessage::ClientInit(value)
  }
}

pub fn md5<T: AsRef<[u8]>>(data: T) -> String {
  let digest = md5::compute(data);
  format!("{:x}", digest)
}

#[derive(Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub struct BroadcastUpdateMessage {
  origin: CollabOrigin,
  object_id: String,
  payload: Vec<u8>,
}

impl BroadcastUpdateMessage {
  pub fn new(origin: CollabOrigin, object_id: String, payload: Vec<u8>) -> Self {
    Self {
      origin,
      object_id,
      payload,
    }
  }
}

impl From<BroadcastUpdateMessage> for CollabMessage {
  fn from(value: BroadcastUpdateMessage) -> Self {
    CollabMessage::BroadcastUpdate(value)
  }
}