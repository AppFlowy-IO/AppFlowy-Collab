use collab::core::collab_awareness::MutexCollabAwareness;

use collab::core::collab::CollabOrigin;
use y_sync::awareness::Awareness;
use y_sync::sync::{Error, Message, Protocol, SyncMessage};
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
/// A implementation of y-sync [Protocol].
pub struct CollabSyncProtocol {
  pub origin: CollabOrigin,
}

impl Protocol for CollabSyncProtocol {
  /// To be called whenever a new connection has been accepted. Returns an encoded list of
  /// messages to be send back to initiator. This binary may contain multiple messages inside,
  /// stored one after another.
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
    awareness: &mut Awareness,
    update: Update,
  ) -> Result<Option<Message>, Error> {
    let mut txn = awareness.doc().transact_mut_with(self.origin.clone());
    // let mut txn = awareness.doc().transact_mut();
    txn.apply_update(update);
    Ok(None)
  }

  /// Handle continuous update send from the client. By default just apply an update to a current
  /// `awareness` document instance.
  fn handle_update(
    &self,
    awareness: &mut Awareness,
    update: Update,
  ) -> Result<Option<Message>, Error> {
    self.handle_sync_step2(awareness, update)
  }
}

/// Handles incoming messages from the client/server
pub async fn handle_msg<P: Protocol>(
  protocol: &P,
  awareness: &MutexCollabAwareness,
  msg: Message,
) -> Result<Option<Message>, Error> {
  match msg {
    Message::Sync(msg) => match msg {
      SyncMessage::SyncStep1(sv) => {
        let awareness = awareness.lock();
        protocol.handle_sync_step1(&awareness, sv)
      },
      SyncMessage::SyncStep2(update) => {
        let mut awareness = awareness.lock();
        protocol.handle_sync_step2(&mut awareness, Update::decode_v1(&update)?)
      },
      SyncMessage::Update(update) => {
        let mut awareness = awareness.lock();
        protocol.handle_update(&mut awareness, Update::decode_v1(&update)?)
      },
    },
    Message::Auth(reason) => {
      let awareness = awareness.lock();
      protocol.handle_auth(&awareness, reason)
    },
    Message::AwarenessQuery => {
      let awareness = awareness.lock();
      protocol.handle_awareness_query(&awareness)
    },
    Message::Awareness(update) => {
      let mut awareness = awareness.lock();
      protocol.handle_awareness_update(&mut awareness, update)
    },
    Message::Custom(tag, data) => {
      let mut awareness = awareness.lock();
      protocol.missing_handle(&mut awareness, tag, data)
    },
  }
}
