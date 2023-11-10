use crate::core::collab::{MutexCollab, TransactionMutExt};
use crate::core::origin::CollabOrigin;
use crate::sync_protocol::awareness::{Awareness, AwarenessUpdate};
use crate::sync_protocol::message::{Error, Message, SyncMessage};
use yrs::updates::decoder::Decode;
use yrs::updates::encoder::{Encode, Encoder, EncoderV1};
use yrs::{ReadTxn, StateVector, Transact, Update};

// ***************************
// Client A  Client B  Server
// |          |             |
// |---(1)--Sync Step1----->|
// |          |             |
// |<--(2)--Sync Step2------|
// |<-------Sync Step1------|
// |          |             |
// |---(3)--Sync Step2----->|
// |          |             |
// **************************
// |---(1)-- Update-------->|
// |          |             |
// |          |  (2) Apply->|
// |          |             |
// |          |<-(3) Broadcast
// |          |             |
// |          |< (4) Apply  |
/// A implementation of [CollabSyncProtocol].
#[derive(Clone)]
pub struct ClientSyncProtocol;
impl CollabSyncProtocol for ClientSyncProtocol {}

#[derive(Clone)]
pub struct ServerSyncProtocol;
impl CollabSyncProtocol for ServerSyncProtocol {
  fn handle_sync_step1(
    &self,
    awareness: &Awareness,
    sv: StateVector,
  ) -> Result<Option<Vec<u8>>, Error> {
    let txn = awareness.doc().transact();
    let step2_update = txn.encode_state_as_update_v1(&sv);
    let step1_update = txn.state_vector();

    let mut encoder = EncoderV1::new();
    Message::Sync(SyncMessage::SyncStep2(step2_update)).encode(&mut encoder);
    Message::Sync(SyncMessage::SyncStep1(step1_update)).encode(&mut encoder);
    Ok(Some(encoder.to_vec()))
  }
}
pub trait CollabSyncProtocol {
  fn start<E: Encoder>(&self, awareness: &Awareness, encoder: &mut E) -> Result<(), Error> {
    let (sv, update) = {
      let sv = awareness.doc().transact().state_vector();
      let update = awareness.update()?;
      (sv, update)
    };
    Message::Sync(SyncMessage::SyncStep1(sv)).encode(encoder);
    Message::Awareness(update).encode(encoder);
    Ok(())
  }

  /// Given a [StateVector] of a remote side, calculate missing
  /// updates. Returns a sync-step-2 message containing a calculated update.
  fn handle_sync_step1(
    &self,
    awareness: &Awareness,
    sv: StateVector,
  ) -> Result<Option<Vec<u8>>, Error> {
    let update = awareness
      .doc()
      .try_transact()
      .map_err(|err| Error::YrsTransaction(err.to_string()))?
      .encode_state_as_update_v1(&sv);
    Ok(Some(
      Message::Sync(SyncMessage::SyncStep2(update)).encode_v1(),
    ))
  }

  /// Handle reply for a sync-step-1 send from this replica previously. By default just apply
  /// an update to current `awareness` document instance.
  fn handle_sync_step2(
    &self,
    origin: &Option<&CollabOrigin>,
    awareness: &mut Awareness,
    update: Update,
  ) -> Result<Option<Vec<u8>>, Error> {
    let mut txn = match origin {
      Some(origin) => awareness.doc().try_transact_mut_with((*origin).clone()),
      None => awareness.doc().try_transact_mut(),
    }
    .map_err(|err| Error::YrsTransaction(err.to_string()))?;
    txn
      .try_apply_update(update)
      .map_err(|err| Error::YrsTransaction(err.to_string()))?;
    Ok(None)
  }

  /// Handle continuous update send from the client. By default just apply an update to a current
  /// `awareness` document instance.
  fn handle_update(
    &self,
    origin: &Option<&CollabOrigin>,
    awareness: &mut Awareness,
    update: Update,
  ) -> Result<Option<Vec<u8>>, Error> {
    self.handle_sync_step2(origin, awareness, update)
  }

  fn handle_auth(
    &self,
    _awareness: &Awareness,
    deny_reason: Option<String>,
  ) -> Result<Option<Vec<u8>>, Error> {
    if let Some(reason) = deny_reason {
      Err(Error::PermissionDenied { reason })
    } else {
      Ok(None)
    }
  }

  /// Returns an [AwarenessUpdate] which is a serializable representation of a current `awareness`
  /// instance.
  fn handle_awareness_query(&self, awareness: &Awareness) -> Result<Option<Vec<u8>>, Error> {
    let update = awareness.update()?;
    Ok(Some(Message::Awareness(update).encode_v1()))
  }

  /// Reply to awareness query or just incoming [AwarenessUpdate], where current `awareness`
  /// instance is being updated with incoming data.
  fn handle_awareness_update(
    &self,
    awareness: &mut Awareness,
    update: AwarenessUpdate,
  ) -> Result<Option<Vec<u8>>, Error> {
    awareness.apply_update(update)?;
    Ok(None)
  }

  fn missing_handle(
    &self,
    _awareness: &mut Awareness,
    tag: u8,
    _data: Vec<u8>,
  ) -> Result<Option<Vec<u8>>, Error> {
    Err(Error::Unsupported(tag))
  }
}

/// Handles incoming messages from the client/server
pub fn handle_msg<P: CollabSyncProtocol>(
  origin: &Option<&CollabOrigin>,
  protocol: &P,
  collab: &MutexCollab,
  msg: Message,
) -> Result<Option<Vec<u8>>, Error> {
  match msg {
    Message::Sync(msg) => match msg {
      SyncMessage::SyncStep1(sv) => {
        let collab = collab.lock();
        protocol.handle_sync_step1(collab.get_awareness(), sv)
      },
      SyncMessage::SyncStep2(update) => {
        let mut collab = collab.lock();
        protocol.handle_sync_step2(
          origin,
          collab.get_mut_awareness(),
          Update::decode_v1(&update)?,
        )
      },
      SyncMessage::Update(update) => {
        let mut collab = collab.lock();
        protocol.handle_update(
          origin,
          collab.get_mut_awareness(),
          Update::decode_v1(&update)?,
        )
      },
    },
    Message::Auth(reason) => {
      let collab = collab.lock();
      protocol.handle_auth(collab.get_awareness(), reason)
    },
    Message::AwarenessQuery => {
      let collab = collab.lock();
      protocol.handle_awareness_query(collab.get_awareness())
    },
    Message::Awareness(update) => {
      let mut collab = collab.lock();
      protocol.handle_awareness_update(collab.get_mut_awareness(), update)
    },
    Message::Custom(tag, data) => {
      let mut collab = collab.lock();
      protocol.missing_handle(collab.get_mut_awareness(), tag, data)
    },
  }
}
