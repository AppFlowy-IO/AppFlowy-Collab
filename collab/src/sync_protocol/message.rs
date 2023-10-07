use crate::sync_protocol::awareness::AwarenessUpdate;
use std::fmt::{Debug, Display, Formatter};
use thiserror::Error;
use yrs::updates::decoder::{Decode, Decoder};
use yrs::updates::encoder::{Encode, Encoder};
use yrs::StateVector;

/// Tag id for [Message::Sync].
pub const MSG_SYNC: u8 = 0;
/// Tag id for [Message::Awareness].
pub const MSG_AWARENESS: u8 = 1;
/// Tag id for [Message::Auth].
pub const MSG_AUTH: u8 = 2;
/// Tag id for [Message::AwarenessQuery].
pub const MSG_QUERY_AWARENESS: u8 = 3;

pub const PERMISSION_DENIED: u8 = 0;
pub const PERMISSION_GRANTED: u8 = 1;

#[derive(Debug, Eq, PartialEq)]
pub enum Message {
  Sync(SyncMessage),
  Auth(Option<String>),
  AwarenessQuery,
  Awareness(AwarenessUpdate),
  Custom(u8, Vec<u8>),
}

impl Encode for Message {
  fn encode<E: Encoder>(&self, encoder: &mut E) {
    match self {
      Message::Sync(msg) => {
        encoder.write_var(MSG_SYNC);
        msg.encode(encoder);
      },
      Message::Auth(reason) => {
        encoder.write_var(MSG_AUTH);
        if let Some(reason) = reason {
          encoder.write_var(PERMISSION_DENIED);
          encoder.write_string(reason);
        } else {
          encoder.write_var(PERMISSION_GRANTED);
        }
      },
      Message::AwarenessQuery => {
        encoder.write_var(MSG_QUERY_AWARENESS);
      },
      Message::Awareness(update) => {
        encoder.write_var(MSG_AWARENESS);
        encoder.write_buf(&update.encode_v1())
      },
      Message::Custom(tag, data) => {
        encoder.write_u8(*tag);
        encoder.write_buf(data);
      },
    }
  }
}

impl Decode for Message {
  fn decode<D: Decoder>(decoder: &mut D) -> Result<Self, lib0::error::Error> {
    let tag: u8 = decoder.read_var()?;
    match tag {
      MSG_SYNC => {
        let msg = SyncMessage::decode(decoder)?;
        Ok(Message::Sync(msg))
      },
      MSG_AWARENESS => {
        let data = decoder.read_buf()?;
        let update = AwarenessUpdate::decode_v1(data)?;
        Ok(Message::Awareness(update))
      },
      MSG_AUTH => {
        let reason = if decoder.read_var::<u8>()? == PERMISSION_DENIED {
          Some(decoder.read_string()?.to_string())
        } else {
          None
        };
        Ok(Message::Auth(reason))
      },
      MSG_QUERY_AWARENESS => Ok(Message::AwarenessQuery),
      tag => {
        let data = decoder.read_buf()?;
        Ok(Message::Custom(tag, data.to_vec()))
      },
    }
  }
}

impl Display for Message {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    match self {
      Message::Sync(sync_msg) => f.write_str(&sync_msg.to_string()),
      Message::Auth(_) => f.write_str("Auth"),
      Message::AwarenessQuery => f.write_str("AwarenessQuery"),
      Message::Awareness(_) => f.write_str("Awareness"),
      Message::Custom(_, _) => f.write_str("Custom"),
    }
  }
}

/// Tag id for [SyncMessage::SyncStep1].
pub const MSG_SYNC_STEP_1: u8 = 0;
/// Tag id for [SyncMessage::SyncStep2].
pub const MSG_SYNC_STEP_2: u8 = 1;
/// Tag id for [SyncMessage::Update].
pub const MSG_SYNC_UPDATE: u8 = 2;

#[derive(Debug, PartialEq, Eq)]
pub enum SyncMessage {
  SyncStep1(StateVector),
  SyncStep2(Vec<u8>),
  Update(Vec<u8>),
}

impl Display for SyncMessage {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    match self {
      SyncMessage::SyncStep1(sv) => {
        write!(f, "SyncStep1({:?})", sv)
      },
      SyncMessage::SyncStep2(data) => {
        write!(f, "SyncStep2({})", data.len())
      },
      SyncMessage::Update(data) => {
        write!(f, "Update({})", data.len())
      },
    }
  }
}

impl Encode for SyncMessage {
  fn encode<E: Encoder>(&self, encoder: &mut E) {
    match self {
      SyncMessage::SyncStep1(sv) => {
        encoder.write_var(MSG_SYNC_STEP_1);
        encoder.write_buf(sv.encode_v1());
      },
      SyncMessage::SyncStep2(u) => {
        encoder.write_var(MSG_SYNC_STEP_2);
        encoder.write_buf(u);
      },
      SyncMessage::Update(u) => {
        encoder.write_var(MSG_SYNC_UPDATE);
        encoder.write_buf(u);
      },
    }
  }
}

impl Decode for SyncMessage {
  fn decode<D: Decoder>(decoder: &mut D) -> Result<Self, lib0::error::Error> {
    let tag: u8 = decoder.read_var()?;
    match tag {
      MSG_SYNC_STEP_1 => {
        let buf = decoder.read_buf()?;
        let sv = StateVector::decode_v1(buf)?;
        Ok(SyncMessage::SyncStep1(sv))
      },
      MSG_SYNC_STEP_2 => {
        let buf = decoder.read_buf()?;
        Ok(SyncMessage::SyncStep2(buf.into()))
      },
      MSG_SYNC_UPDATE => {
        let buf = decoder.read_buf()?;
        Ok(SyncMessage::Update(buf.into()))
      },
      _ => Err(lib0::error::Error::UnexpectedValue),
    }
  }
}

#[derive(Debug, Error)]
pub enum Error {
  /// Incoming Y-protocol message couldn't be deserialized.
  #[error("failed to deserialize message: {0}")]
  DecodingError(#[from] lib0::error::Error),

  /// Applying incoming Y-protocol awareness update has failed.
  #[error("failed to process awareness update: {0}")]
  AwarenessEncoding(#[from] crate::sync_protocol::awareness::Error),

  /// An incoming Y-protocol authorization request has been denied.
  #[error("permission denied to access: {reason}")]
  PermissionDenied { reason: String },

  /// Thrown whenever an unknown message tag has been sent.
  #[error("unsupported message tag identifier: {0}")]
  Unsupported(u8),

  /// Custom dynamic kind of error, usually related to a warp internal error messages.
  #[error("internal failure: {0}")]
  Other(#[from] Box<dyn std::error::Error + Send + Sync>),
}

#[cfg(feature = "net")]
impl From<tokio::task::JoinError> for Error {
  fn from(value: tokio::task::JoinError) -> Self {
    Error::Other(value.into())
  }
}

impl From<std::io::Error> for Error {
  fn from(value: std::io::Error) -> Self {
    Error::DecodingError(lib0::error::Error::IO(value))
  }
}

/// [MessageReader] can be used over the decoder to read these messages one by one in iterable
/// fashion.
pub struct MessageReader<'a, D: Decoder>(&'a mut D);

impl<'a, D: Decoder> MessageReader<'a, D> {
  pub fn new(decoder: &'a mut D) -> Self {
    MessageReader(decoder)
  }
}

impl<'a, D: Decoder> Iterator for MessageReader<'a, D> {
  type Item = Result<Message, lib0::error::Error>;

  fn next(&mut self) -> Option<Self::Item> {
    match Message::decode(self.0) {
      Ok(msg) => Some(Ok(msg)),
      Err(lib0::error::Error::EndOfBuffer(_)) => None,
      Err(error) => Some(Err(error)),
    }
  }
}
