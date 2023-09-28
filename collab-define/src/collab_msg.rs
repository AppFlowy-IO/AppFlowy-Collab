use std::cmp::Ordering;
use std::fmt::{Display, Formatter};

use crate::CollabType;
use bytes::Bytes;
use collab::core::origin::CollabOrigin;
use serde::{Deserialize, Serialize};

pub trait CollabSinkMessage: Clone + Send + Sync + 'static + Ord + Display {
  /// Returns the length of the message in bytes.
  fn length(&self) -> usize;
  /// Returns true if the message can be merged with other messages.
  /// Check the implementation of `queue_or_merge_msg` for more details.
  fn mergeable(&self) -> bool;

  fn merge(&mut self, other: Self);

  fn is_init_msg(&self) -> bool;

  /// Determine if the message can be deferred base on the current state of the sink.
  fn deferrable(&self) -> bool;
}

pub type MsgId = u64;
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum CollabMessage {
  ClientInit(ClientCollabInit),
  ClientUpdateRequest(ClientUpdateRequest),
  ClientUpdateResponse(ClientUpdateResponse),
  AwarenessUpdate(CSAwarenessUpdate),
  ServerBroadcast(CollabServerBroadcast),
  // ServerInitResponse(ServerCollabInitResponse),
  // ServerInit(ServerCollabInit),
}

impl CollabSinkMessage for CollabMessage {
  fn length(&self) -> usize {
    self.payload().len()
  }

  fn mergeable(&self) -> bool {
    false
  }

  fn merge(&mut self, _other: Self) {
    // Do nothing. Because mergeable is false.
  }

  fn is_init_msg(&self) -> bool {
    self.is_init()
  }

  fn deferrable(&self) -> bool {
    // If the message is not init, it can be pending.
    !self.is_init()
  }
}

impl Eq for CollabMessage {}

impl PartialEq for CollabMessage {
  fn eq(&self, other: &Self) -> bool {
    self.msg_id() == other.msg_id()
  }
}

impl PartialOrd for CollabMessage {
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
    Some(self.cmp(other))
  }
}

impl Ord for CollabMessage {
  fn cmp(&self, other: &Self) -> Ordering {
    match (&self, &other) {
      (CollabMessage::ClientInit { .. }, CollabMessage::ClientInit { .. }) => Ordering::Equal,
      (CollabMessage::ClientInit { .. }, _) => Ordering::Greater,
      (_, CollabMessage::ClientInit { .. }) => Ordering::Less,
      // (CollabMessage::ServerInitResponse(l_resp), CollabMessage::ServerInitResponse(r_resp)) => {
      //   l_resp.msg_id.cmp(&r_resp.msg_id).reverse()
      // },
      // (CollabMessage::ServerInitResponse(_), _) => Ordering::Greater,
      // (_, CollabMessage::ServerInitResponse { .. }) => Ordering::Less,
      _ => self.msg_id().cmp(&other.msg_id()).reverse(),
    }
  }
}

impl CollabMessage {
  /// Currently, only have one business id. So just return 1.
  pub fn business_id(&self) -> u8 {
    1
  }

  pub fn is_init(&self) -> bool {
    matches!(self, CollabMessage::ClientInit(_))
  }

  pub fn msg_id(&self) -> Option<MsgId> {
    match self {
      CollabMessage::ClientInit(value) => Some(value.msg_id),
      // CollabMessage::ServerInitResponse(value) => Some(value.msg_id),
      CollabMessage::ClientUpdateRequest(value) => Some(value.msg_id),
      CollabMessage::ClientUpdateResponse(value) => value.msg_id,
      CollabMessage::ServerBroadcast(_) => None,
      CollabMessage::AwarenessUpdate(_) => None,
      // CollabMessage::ServerInit(value) => Some(value.msg_id),
    }
  }

  pub fn is_empty(&self) -> bool {
    self.payload().is_empty()
  }

  pub fn origin(&self) -> Option<&CollabOrigin> {
    match self {
      CollabMessage::ClientInit(value) => Some(&value.origin),
      // CollabMessage::ServerInitResponse(value) => Some(&value.origin),
      CollabMessage::ClientUpdateRequest(value) => Some(&value.origin),
      CollabMessage::ClientUpdateResponse(value) => Some(&value.origin),
      CollabMessage::ServerBroadcast(value) => Some(&value.origin),
      CollabMessage::AwarenessUpdate(_) => None,
      // CollabMessage::ServerInit(_) => None,
    }
  }

  pub fn object_id(&self) -> &str {
    match self {
      CollabMessage::ClientInit(value) => &value.object_id,
      // CollabMessage::ServerInitResponse(value) => &value.object_id,
      CollabMessage::ClientUpdateRequest(value) => &value.object_id,
      CollabMessage::ClientUpdateResponse(value) => &value.object_id,
      CollabMessage::ServerBroadcast(value) => &value.object_id,
      CollabMessage::AwarenessUpdate(value) => &value.object_id,
      // CollabMessage::ServerInit(value) => &value.object_id,
    }
  }
}

impl Display for CollabMessage {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    match self {
      CollabMessage::ClientInit(value) => f.write_fmt(format_args!(
        "client init: [{}|oid:{}|payload_len:{}|msg_id:{}]",
        value.origin,
        value.object_id,
        value.payload.len(),
        value.msg_id,
      )),
      // CollabMessage::ServerInitResponse(value) => f.write_fmt(format_args!(
      //   "server init response: [oid:{}|payload_len:{}|msg_id:{}]",
      //   value.object_id,
      //   value.payload.len(),
      //   value.msg_id,
      // )),
      // CollabMessage::ServerInit(value) => f.write_fmt(format_args!(
      //   "server init request: [oid:{}|msg_id:{}]",
      //   value.object_id, value.msg_id,
      // )),
      CollabMessage::ClientUpdateRequest(value) => f.write_fmt(format_args!(
        "client update request: [{}|oid:{}|msg_id:{}|payload_len:{}]",
        value.origin,
        value.object_id,
        value.msg_id,
        value.payload.len(),
      )),
      CollabMessage::ClientUpdateResponse(value) => f.write_fmt(format_args!(
        "client update response: [oid:{}|payload_len:{}]",
        value.object_id,
        value.payload.len(),
      )),
      CollabMessage::ServerBroadcast(value) => f.write_fmt(format_args!(
        "server broadcast: [{}|oid:{}|payload_len:{}]",
        value.origin,
        value.object_id,
        value.payload.len(),
      )),
      CollabMessage::AwarenessUpdate(value) => f.write_fmt(format_args!(
        "awareness: [oid:{}|payload_len:{}]",
        value.object_id,
        value.payload.len(),
      )),
    }
  }
}

impl From<CollabMessage> for Bytes {
  fn from(msg: CollabMessage) -> Self {
    Bytes::from(msg.to_vec())
  }
}

impl CollabMessage {
  pub fn to_vec(&self) -> Vec<u8> {
    serde_json::to_vec(self).unwrap_or_default()
  }

  pub fn from_vec(data: &[u8]) -> Result<Self, serde_json::Error> {
    serde_json::from_slice(data)
  }

  pub fn payload(&self) -> &Bytes {
    match self {
      CollabMessage::ClientInit(value) => &value.payload,
      // CollabMessage::ServerInitResponse(value) => &value.payload,
      CollabMessage::ClientUpdateRequest(value) => &value.payload,
      CollabMessage::ClientUpdateResponse(value) => &value.payload,
      CollabMessage::ServerBroadcast(value) => &value.payload,
      CollabMessage::AwarenessUpdate(value) => &value.payload,
      // CollabMessage::ServerInit(value) => &value.payload,
    }
  }
}

#[derive(Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub struct CSAwarenessUpdate {
  object_id: String,
  payload: Bytes,
}

impl CSAwarenessUpdate {
  pub fn new(object_id: String, payload: Vec<u8>) -> Self {
    Self {
      object_id,
      payload: Bytes::from(payload),
    }
  }
}

impl From<CSAwarenessUpdate> for CollabMessage {
  fn from(value: CSAwarenessUpdate) -> Self {
    CollabMessage::AwarenessUpdate(value)
  }
}

#[derive(Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub struct ClientUpdateRequest {
  origin: CollabOrigin,
  object_id: String,
  msg_id: MsgId,
  payload: Bytes,
}

impl ClientUpdateRequest {
  pub fn new(origin: CollabOrigin, object_id: String, msg_id: MsgId, payload: Vec<u8>) -> Self {
    Self {
      origin,
      object_id,
      msg_id,
      payload: Bytes::from(payload),
    }
  }
}

impl From<ClientUpdateRequest> for CollabMessage {
  fn from(value: ClientUpdateRequest) -> Self {
    CollabMessage::ClientUpdateRequest(value)
  }
}

// #[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
// pub struct ServerCollabInit {
//   pub object_id: String,
//   pub msg_id: MsgId,
//   pub payload: Bytes,
// }
//
// impl ServerCollabInit {
//   pub fn new(object_id: String, msg_id: MsgId, payload: Vec<u8>) -> Self {
//     Self {
//       object_id,
//       msg_id,
//       payload: Bytes::from(payload),
//     }
//   }
// }
//
// impl From<ServerCollabInit> for CollabMessage {
//   fn from(value: ServerCollabInit) -> Self {
//     CollabMessage::ServerInit(value)
//   }
// }

#[derive(Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub struct ClientCollabInit {
  pub origin: CollabOrigin,
  pub object_id: String,
  pub collab_type: CollabType,
  pub workspace_id: String,
  pub msg_id: MsgId,
  pub payload: Bytes,
}

impl ClientCollabInit {
  pub fn new(
    origin: CollabOrigin,
    object_id: String,
    collab_type: CollabType,
    workspace_id: String,
    msg_id: MsgId,
    payload: Vec<u8>,
  ) -> Self {
    let payload = Bytes::from(payload);
    Self {
      origin,
      object_id,
      collab_type,
      workspace_id,
      msg_id,
      payload,
    }
  }
}

impl From<ClientCollabInit> for CollabMessage {
  fn from(value: ClientCollabInit) -> Self {
    CollabMessage::ClientInit(value)
  }
}

#[derive(Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub struct ClientUpdateResponse {
  pub origin: CollabOrigin,
  pub object_id: String,
  pub msg_id: Option<MsgId>,
  pub payload: Bytes,
}

impl ClientUpdateResponse {
  pub fn new(
    origin: CollabOrigin,
    object_id: String,
    payload: Vec<u8>,
    msg_id: Option<MsgId>,
  ) -> Self {
    Self {
      origin,
      object_id,
      payload: Bytes::from(payload),
      msg_id,
    }
  }
}

impl From<ClientUpdateResponse> for CollabMessage {
  fn from(value: ClientUpdateResponse) -> Self {
    CollabMessage::ClientUpdateResponse(value)
  }
}

impl Display for ClientUpdateResponse {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    f.write_fmt(format_args!(
      "client update response: [uid:{:?}|oid:{}|msg_id:{:?}|payload_len:{}]",
      self.origin.client_user_id(),
      self.object_id,
      self.msg_id,
      self.payload.len(),
    ))
  }
}

#[derive(Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub struct CollabServerBroadcast {
  origin: CollabOrigin,
  object_id: String,
  payload: Bytes,
}

impl CollabServerBroadcast {
  pub fn new(origin: CollabOrigin, object_id: String, payload: Vec<u8>) -> Self {
    Self {
      origin,
      object_id,
      payload: Bytes::from(payload),
    }
  }
}

impl From<CollabServerBroadcast> for CollabMessage {
  fn from(value: CollabServerBroadcast) -> Self {
    CollabMessage::ServerBroadcast(value)
  }
}
//
// #[derive(Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
// pub struct ServerCollabInitResponse {
//   // Indicates the origin client of the message
//   pub origin: CollabOrigin,
//   pub object_id: String,
//   pub payload: Bytes,
//   pub msg_id: MsgId,
// }
//
// impl ServerCollabInitResponse {
//   pub fn new(origin: CollabOrigin, object_id: String, payload: Vec<u8>, msg_id: MsgId) -> Self {
//     Self {
//       origin,
//       object_id,
//       payload: Bytes::from(payload),
//       msg_id,
//     }
//   }
// }
//
// impl From<ServerCollabInitResponse> for CollabMessage {
//   fn from(value: ServerCollabInitResponse) -> Self {
//     CollabMessage::ServerInitResponse(value)
//   }
// }
