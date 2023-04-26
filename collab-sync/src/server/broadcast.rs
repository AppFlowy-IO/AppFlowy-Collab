use crate::error::SyncError;
use crate::protocol::{handle_msg, CollabSyncProtocol};
use futures_util::{SinkExt, StreamExt};
use lib0::encoding::Write;
use std::sync::Arc;
use tokio::select;
use tokio::sync::broadcast::error::SendError;
use tokio::sync::broadcast::{channel, Receiver, Sender};
use tokio::sync::{Mutex, RwLock};
use tokio::task::JoinHandle;
use y_sync::awareness;
use y_sync::awareness::{Awareness, AwarenessUpdate};
use y_sync::sync::{Message, MSG_SYNC, MSG_SYNC_UPDATE};
use yrs::updates::decoder::Decode;
use yrs::updates::encoder::{Encode, Encoder, EncoderV1};
use yrs::UpdateSubscription;

/// A broadcast group can be used to propagate updates produced by yrs [yrs::Doc] and [Awareness]
/// to subscribes.
pub struct BroadcastGroup {
  awareness_sub: awareness::UpdateSubscription,
  doc_sub: UpdateSubscription,
  awareness: Arc<RwLock<Awareness>>,
  sender: Sender<Vec<u8>>,
  receiver: Receiver<Vec<u8>>,
}

impl BroadcastGroup {
  /// Creates a new [BroadcastGroup] over a provided `awareness` instance. All changes triggered
  /// by this awareness structure or its underlying document will be propagated to all subscribers
  /// which have been registered via [BroadcastGroup::subscribe] method.
  ///
  /// The overflow of the incoming events that needs to be propagates will be buffered up to a
  /// provided `buffer_capacity` size.
  pub async fn new(awareness: Arc<RwLock<Awareness>>, buffer_capacity: usize) -> Self {
    let (sender, receiver) = channel(buffer_capacity);
    let (doc_sub, awareness_sub) = {
      let mut awareness = awareness.write().await;

      // Observer the document's update and broadcast it to all subscribers.
      let sink = sender.clone();
      let doc_sub = awareness
        .doc_mut()
        .observe_update_v1(move |_txn, event| {
          if let Err(_e) = sink.send(gen_update_message(&event.update)) {
            tracing::trace!("Broadcast group is closed");
          }
        })
        .unwrap();

      // Observer the awareness's update and broadcast it to all subscribers.
      let sink = sender.clone();
      let awareness_sub = awareness.on_update(move |awareness, event| {
        if let Ok(u) = gen_awareness_update_message(awareness, event) {
          let msg = Message::Awareness(u).encode_v1();
          if let Err(_e) = sink.send(msg) {
            tracing::trace!("Broadcast group is closed");
          }
        }
      });
      (doc_sub, awareness_sub)
    };
    BroadcastGroup {
      awareness,
      sender,
      receiver,
      awareness_sub,
      doc_sub,
    }
  }

  /// Returns a reference to an underlying [Awareness] instance.
  pub fn awareness(&self) -> &Arc<RwLock<Awareness>> {
    &self.awareness
  }

  /// Broadcasts user message to all active subscribers. Returns error if message could not have
  /// been broadcast.
  pub fn broadcast(&self, msg: Vec<u8>) -> Result<(), SendError<Vec<u8>>> {
    self.sender.send(msg)?;
    Ok(())
  }

  /// Subscribes a new connection - represented by `sink`/`stream` pair implementing a futures
  /// Sink and Stream protocols - to a current broadcast group.
  ///
  /// Returns a subscription structure, which can be dropped in order to unsubscribe or awaited
  /// via [Subscription::completed] method in order to complete of its own volition (due to
  /// an internal connection error or closed connection).
  pub fn subscribe<Sink, Stream, E>(
    &self,
    sink: Arc<Mutex<Sink>>,
    mut stream: Stream,
  ) -> Subscription
  where
    Sink: SinkExt<Vec<u8>> + Send + Sync + Unpin + 'static,
    Stream: StreamExt<Item = Result<Vec<u8>, E>> + Send + Sync + Unpin + 'static,
    <Sink as futures_util::Sink<Vec<u8>>>::Error: std::error::Error + Send + Sync,
    E: std::error::Error + Send + Sync + 'static,
  {
    // Forward the message to the subscribers
    let sink_task = {
      let sink = sink.clone();
      let mut receiver = self.sender.subscribe();
      tokio::spawn(async move {
        while let Ok(msg) = receiver.recv().await {
          let mut sink = sink.lock().await;
          if let Err(e) = sink.send(msg).await {
            println!("broadcast failed to sent sync message");
            return Err(SyncError::Internal(Box::new(e)));
          }
        }
        Ok(())
      })
    };

    // Receive the message from the subscriber and reply with the response
    let stream_task = {
      let awareness = self.awareness().clone();
      tokio::spawn(async move {
        while let Some(res) = stream.next().await {
          let msg = Message::decode_v1(&res.map_err(|e| SyncError::Internal(Box::new(e)))?)?;
          let reply = handle_msg(&CollabSyncProtocol, &awareness, msg).await?;
          match reply {
            None => {},
            Some(reply) => {
              let mut sink = sink.lock().await;
              sink
                .send(reply.encode_v1())
                .await
                .map_err(|e| SyncError::Internal(Box::new(e)))?;
            },
          }
        }
        Ok(())
      })
    };

    Subscription {
      sink_task,
      stream_task,
    }
  }
}

/// A subscription structure returned from [BroadcastGroup::subscribe], which represents a
/// subscribed connection. It can be dropped in order to unsubscribe or awaited via
/// [Subscription::completed] method in order to complete of its own volition (due to an internal
/// connection error or closed connection).
#[derive(Debug)]
pub struct Subscription {
  sink_task: JoinHandle<Result<(), SyncError>>,
  stream_task: JoinHandle<Result<(), SyncError>>,
}

impl Subscription {
  /// Consumes current subscription, waiting for it to complete. If an underlying connection was
  /// closed because of failure, an error which caused it to happen will be returned.
  ///
  /// This method doesn't invoke close procedure. If you need that, drop current subscription instead.
  pub async fn completed(self) -> Result<(), SyncError> {
    let res = select! {
        r1 = self.sink_task => r1?,
        r2 = self.stream_task => r2?,
    };
    res
  }
}

fn gen_update_message(update: &[u8]) -> Vec<u8> {
  let mut encoder = EncoderV1::new();
  encoder.write_var(MSG_SYNC);
  encoder.write_var(MSG_SYNC_UPDATE);
  encoder.write_buf(update);
  encoder.to_vec()
}

fn gen_awareness_update_message(
  awareness: &Awareness,
  event: &awareness::Event,
) -> Result<AwarenessUpdate, SyncError> {
  let added = event.added();
  let updated = event.updated();
  let removed = event.removed();
  let mut changed = Vec::with_capacity(added.len() + updated.len() + removed.len());
  changed.extend_from_slice(added);
  changed.extend_from_slice(updated);
  changed.extend_from_slice(removed);
  let update = awareness.update_with_clients(changed)?;
  Ok(update)
}
