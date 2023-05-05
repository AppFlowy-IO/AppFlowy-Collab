use crate::error::SyncError;
use crate::msg::CollabMessage;
use crate::server::{BroadcastGroup, Subscription};
use bytes::{Bytes, BytesMut};
use collab::core::collab_awareness::MutexCollab;
use collab::preclude::Collab;
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio_util::codec::{Decoder, Encoder, FramedRead, FramedWrite, LengthDelimitedCodec};

pub struct CollabGroup {
  pub mutex_collab: MutexCollab,
  pub broadcast: BroadcastGroup,
  pub subscriptions: Vec<Subscription>,
}

impl CollabGroup {
  pub fn mut_collab<F>(&self, f: F)
  where
    F: FnOnce(&Collab),
  {
    let collab = self.mutex_collab.lock();
    f(&collab);
  }
}

#[derive(Debug, Default)]
pub struct CollabMsgCodec(LengthDelimitedCodec);

impl Encoder<CollabMessage> for CollabMsgCodec {
  type Error = SyncError;

  fn encode(&mut self, item: CollabMessage, dst: &mut BytesMut) -> Result<(), Self::Error> {
    let bytes = item.to_vec();
    self.0.encode(Bytes::from(bytes), dst)?;
    Ok(())
  }
}

impl Decoder for CollabMsgCodec {
  type Item = CollabMessage;
  type Error = SyncError;

  fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
    if let Some(bytes) = self.0.decode(src)? {
      let bytes = bytes.freeze().to_vec();
      let msg = CollabMessage::from_vec(bytes).ok();
      Ok(msg)
    } else {
      Ok(None)
    }
  }
}

pub type CollabStream = FramedRead<OwnedReadHalf, CollabMsgCodec>;
pub type CollabSink = FramedWrite<OwnedWriteHalf, CollabMsgCodec>;
