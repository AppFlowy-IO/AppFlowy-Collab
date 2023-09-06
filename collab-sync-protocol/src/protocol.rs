use collab::core::collab::MutexCollab;
use collab::core::origin::CollabOrigin;
use y_sync::awareness::{Awareness, AwarenessUpdate};
use y_sync::sync::{Error, Message, SyncMessage};
use yrs::updates::decoder::Decode;
use yrs::updates::encoder::{Encode, Encoder};
use yrs::{ReadTxn, StateVector, Transact, Update};

// In a client-server model. The client is the initiator of the connection. The server is the
// responder. The client sends a sync-step-1 message to the server. The server responds with a
// sync-step-2 message. The client then sends an update message to the server. The server applies
// the update.
// ********************************
// Client A  Client B  Server
// |        |             |
// |---(1) Sync Step1--->
// |        |             |
// |<--(2) Sync Step2---  |
// |        |             |
// |---(3) Update------>  |
// |        |             |
// |        |---(4) Apply Update
// |        |             |
// ********************************
// |        |---(5) Sync Step1--->
// |        |             |
// |        |<--(6) Sync Step2---|
// |        |             |
// |---(7) Update------>  |
// |        |             |
// |        |---(8) Apply Update
// |        |             |
// |        |<-(9) Broadcast Update
// |        |             |
// ********************************
/// A implementation of [CollabSyncProtocol].
#[derive(Clone)]
pub struct DefaultSyncProtocol;

impl CollabSyncProtocol for DefaultSyncProtocol {}

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
  ) -> Result<Option<Message>, Error> {
    let update = awareness.doc().transact().encode_state_as_update_v1(&sv);
    Ok(Some(Message::Sync(SyncMessage::SyncStep2(update))))
  }

  /// Handle reply for a sync-step-1 send from this replica previously. By default just apply
  /// an update to current `awareness` document instance.
  fn handle_sync_step2(
    &self,
    origin: &Option<&CollabOrigin>,
    awareness: &mut Awareness,
    update: Update,
  ) -> Result<Option<Message>, Error> {
    let mut txn = match origin {
      Some(origin) => awareness.doc().transact_mut_with((*origin).clone()),
      None => awareness.doc().transact_mut(),
    };
    txn.apply_update(update);
    Ok(None)
  }

  /// Handle continuous update send from the client. By default just apply an update to a current
  /// `awareness` document instance.
  fn handle_update(
    &self,
    origin: &Option<&CollabOrigin>,
    awareness: &mut Awareness,
    update: Update,
  ) -> Result<Option<Message>, Error> {
    self.handle_sync_step2(origin, awareness, update)
  }

  fn handle_auth(
    &self,
    _awareness: &Awareness,
    deny_reason: Option<String>,
  ) -> Result<Option<Message>, Error> {
    if let Some(reason) = deny_reason {
      Err(Error::PermissionDenied { reason })
    } else {
      Ok(None)
    }
  }

  /// Returns an [AwarenessUpdate] which is a serializable representation of a current `awareness`
  /// instance.
  fn handle_awareness_query(&self, awareness: &Awareness) -> Result<Option<Message>, Error> {
    let update = awareness.update()?;
    Ok(Some(Message::Awareness(update)))
  }

  /// Reply to awareness query or just incoming [AwarenessUpdate], where current `awareness`
  /// instance is being updated with incoming data.
  fn handle_awareness_update(
    &self,
    awareness: &mut Awareness,
    update: AwarenessUpdate,
  ) -> Result<Option<Message>, Error> {
    awareness.apply_update(update)?;
    Ok(None)
  }

  /// Y-sync protocol enables to extend its own settings with custom handles. These can be
  /// implemented here. By default it returns an [Error::Unsupported].
  fn missing_handle(
    &self,
    _awareness: &mut Awareness,
    tag: u8,
    _data: Vec<u8>,
  ) -> Result<Option<Message>, Error> {
    Err(Error::Unsupported(tag))
  }
}

/// Handles incoming messages from the client/server
pub async fn handle_msg<P: CollabSyncProtocol>(
  origin: &Option<&CollabOrigin>,
  protocol: &P,
  collab: &MutexCollab,
  msg: Message,
) -> Result<Option<Message>, Error> {
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
