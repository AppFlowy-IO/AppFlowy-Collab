use std::fmt::{Display, Formatter};

use collab::core::collab::CollabOrigin;
use serde::{Deserialize, Serialize};

use crate::error::SyncError;

#[derive(Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub enum CollabMessage {
  ClientInit(ClientInitMessage),
  ServerSync(ServerSyncMessage),
  ClientUpdate(ClientUpdateMessage),
  AwarenessUpdate(AwarenessUpdateMessage),
  ServerResponse(ServerResponseMessage),
  ServerBroadcast(ServerBroadcastMessage),
  ServerAck(CollabAckMessage),
}

impl CollabMessage {
  pub fn is_init(&self) -> bool {
    matches!(self, CollabMessage::ClientInit(_))
  }

  pub fn msg_id(&self) -> Option<u32> {
    match self {
      CollabMessage::ClientInit(value) => Some(value.msg_id),
      CollabMessage::ServerSync(value) => Some(value.msg_id),
      CollabMessage::ClientUpdate(value) => Some(value.msg_id),
      CollabMessage::ServerResponse(_) => None,
      CollabMessage::ServerBroadcast(_) => None,
      CollabMessage::AwarenessUpdate(_) => None,
      CollabMessage::ServerAck(value) => Some(value.msg_id),
    }
  }

  pub fn is_empty(&self) -> bool {
    match self {
      CollabMessage::ClientInit(value) => value.payload.is_empty(),
      CollabMessage::ServerSync(value) => value.payload.is_empty(),
      CollabMessage::ClientUpdate(value) => value.payload.is_empty(),
      CollabMessage::ServerResponse(value) => value.payload.is_empty(),
      CollabMessage::ServerBroadcast(value) => value.payload.is_empty(),
      CollabMessage::AwarenessUpdate(value) => value.payload.is_empty(),
      CollabMessage::ServerAck(value) => match value.payload {
        Some(ref payload) => payload.is_empty(),
        None => true,
      },
    }
  }

  pub fn origin(&self) -> CollabOrigin {
    match self {
      CollabMessage::ClientInit(value) => value.origin.clone(),
      CollabMessage::ServerSync(value) => value.origin.clone(),
      CollabMessage::ClientUpdate(value) => value.origin.clone(),
      CollabMessage::ServerResponse(value) => value.origin.clone(),
      CollabMessage::ServerBroadcast(value) => value.origin.clone(),
      CollabMessage::AwarenessUpdate(_) => CollabOrigin::default(),
      CollabMessage::ServerAck(_) => CollabOrigin::default(),
    }
  }
}

impl Display for CollabMessage {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    match self {
      CollabMessage::ClientInit(value) => f.write_fmt(format_args!(
        "client init: [uid:{}|device_id:{}|oid:{}|payload_len:{}|msg_id:{}]",
        value.origin.uid,
        value.origin.device_id,
        value.object_id,
        value.payload.len(),
        value.msg_id,
      )),
      CollabMessage::ServerSync(value) => f.write_fmt(format_args!(
        "sync state: [oid:{}|payload_len:{}|msg_id:{}]",
        value.object_id,
        value.payload.len(),
        value.msg_id,
      )),
      CollabMessage::ClientUpdate(value) => f.write_fmt(format_args!(
        "send client update: [uid:{}|device_id:{}|oid:{}|payload_len:{}|msg_id:{}]",
        value.origin.uid,
        value.origin.device_id,
        value.object_id,
        value.payload.len(),
        value.msg_id,
      )),
      CollabMessage::ServerResponse(value) => f.write_fmt(format_args!(
        "server response: [uid:{}|device_id:{}|oid:{}|payload_len:{}]",
        value.origin.uid,
        value.origin.device_id,
        value.object_id,
        value.payload.len(),
      )),
      CollabMessage::ServerBroadcast(value) => f.write_fmt(format_args!(
        "broadcast update: [uid:{}|device_id:{}|oid:{}|payload_len:{}]",
        value.origin.uid,
        value.origin.device_id,
        value.object_id,
        value.payload.len(),
      )),
      CollabMessage::AwarenessUpdate(value) => f.write_fmt(format_args!(
        "awareness: [oid:{}|payload_len:{}]",
        value.object_id,
        value.payload.len(),
      )),
      CollabMessage::ServerAck(value) => f.write_fmt(format_args!(
        "ack message: [oid:{}|msg_id:{}]",
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
      CollabMessage::ServerSync(value) => value.payload,
      CollabMessage::ClientUpdate(value) => value.payload,
      CollabMessage::ServerResponse(value) => value.payload,
      CollabMessage::ServerBroadcast(value) => value.payload,
      CollabMessage::AwarenessUpdate(value) => value.payload,
      CollabMessage::ServerAck(value) => match value.payload {
        Some(payload) => payload,
        None => vec![],
      },
    }
  }

  pub fn payload(&self) -> Option<&Vec<u8>> {
    match self {
      CollabMessage::ClientInit(value) => Some(&value.payload),
      CollabMessage::ServerSync(value) => Some(&value.payload),
      CollabMessage::ClientUpdate(value) => Some(&value.payload),
      CollabMessage::ServerResponse(value) => Some(&value.payload),
      CollabMessage::ServerBroadcast(value) => Some(&value.payload),
      CollabMessage::AwarenessUpdate(value) => Some(&value.payload),
      CollabMessage::ServerAck(value) => value.payload.as_ref(),
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
  pub payload: Option<Vec<u8>>,
}

impl CollabAckMessage {
  pub fn new(object_id: String, msg_id: u32, payload: Option<Vec<u8>>) -> Self {
    Self {
      object_id,
      msg_id,
      payload,
    }
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
pub struct ServerResponseMessage {
  origin: CollabOrigin,
  object_id: String,
  payload: Vec<u8>,
}

impl ServerResponseMessage {
  pub fn new(origin: CollabOrigin, object_id: String, payload: Vec<u8>) -> Self {
    Self {
      origin,
      object_id,
      payload,
    }
  }
}

impl From<ServerResponseMessage> for CollabMessage {
  fn from(value: ServerResponseMessage) -> Self {
    CollabMessage::ServerResponse(value)
  }
}

#[derive(Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub struct ServerBroadcastMessage {
  origin: CollabOrigin,
  object_id: String,
  payload: Vec<u8>,
}

impl ServerBroadcastMessage {
  pub fn new(origin: CollabOrigin, object_id: String, payload: Vec<u8>) -> Self {
    Self {
      origin,
      object_id,
      payload,
    }
  }
}

impl From<ServerBroadcastMessage> for CollabMessage {
  fn from(value: ServerBroadcastMessage) -> Self {
    CollabMessage::ServerBroadcast(value)
  }
}

#[derive(Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub struct ServerSyncMessage {
  pub origin: CollabOrigin,
  pub object_id: String,
  pub payload: Vec<u8>,
  pub msg_id: u32,
}

impl ServerSyncMessage {
  pub fn new(origin: CollabOrigin, object_id: String, payload: Vec<u8>, msg_id: u32) -> Self {
    Self {
      origin,
      object_id,
      payload,
      msg_id,
    }
  }
}

impl From<ServerSyncMessage> for CollabMessage {
  fn from(value: ServerSyncMessage) -> Self {
    CollabMessage::ServerSync(value)
  }
}
