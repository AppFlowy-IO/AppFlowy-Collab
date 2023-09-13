use std::marker::PhantomData;
use std::ops::Deref;
use std::sync::{Arc, Weak};

use collab::core::collab::MutexCollab;
use collab::core::origin::CollabOrigin;
use futures_util::{SinkExt, StreamExt};
use lib0::decoding::Cursor;
use tokio::spawn;
use tokio::sync::watch;
use tokio::task::JoinHandle;
use y_sync::awareness::Awareness;
use y_sync::sync::{Message, MessageReader};
use yrs::updates::decoder::{Decode, DecoderV1};
use yrs::updates::encoder::{Encode, Encoder, EncoderV1};

use crate::sync_plugin::client::SyncError;
use crate::sync_plugin::client::{
  CollabSink, CollabSinkRunner, DefaultMsgIdCounter, SinkConfig, SinkState,
};
use crate::sync_plugin::SyncObject;
use collab_sync_protocol::{handle_msg, CollabSyncProtocol, DefaultSyncProtocol};
use collab_sync_protocol::{ClientCollabInit, ClientCollabUpdate, CollabMessage, CollabServerSync};

pub const DEFAULT_SYNC_TIMEOUT: u64 = 2;

pub struct SyncQueue<Sink, Stream> {
  object: SyncObject,
  origin: CollabOrigin,
  /// The [CollabSink] is used to send the updates to the remote. It will send the current
  /// update periodically if the timeout is reached or it will send the next update if
  /// it receive previous ack from the remote.
  sink: Arc<CollabSink<Sink, CollabMessage>>,
  /// The [SyncStream] will be spawned in a separate task It continuously receive
  /// the updates from the remote.
  #[allow(dead_code)]
  stream: SyncStream<Sink, Stream>,
  protocol: DefaultSyncProtocol,
}

impl<E, Sink, Stream> SyncQueue<Sink, Stream>
where
  E: std::error::Error + Send + Sync + 'static,
  Sink: SinkExt<CollabMessage, Error = E> + Send + Sync + Unpin + 'static,
  Stream: StreamExt<Item = Result<CollabMessage, E>> + Send + Sync + Unpin + 'static,
{
  pub fn new(
    object: SyncObject,
    origin: CollabOrigin,
    sink: Sink,
    stream: Stream,
    collab: Weak<MutexCollab>,
    config: SinkConfig,
  ) -> Self {
    let protocol = DefaultSyncProtocol;
    let (notifier, notifier_rx) = watch::channel(false);
    let (state_tx, _) = watch::channel(SinkState::Init);
    debug_assert!(origin.client_user_id().is_some());

    let sink = Arc::new(CollabSink::new(
      origin.client_user_id().unwrap_or(0),
      sink,
      notifier,
      state_tx,
      DefaultMsgIdCounter::new(),
      config,
    ));

    spawn(CollabSinkRunner::run(Arc::downgrade(&sink), notifier_rx));
    let cloned_protocol = protocol.clone();
    let object_id = object.object_id.clone();
    let stream = SyncStream::new(
      origin.clone(),
      object_id,
      stream,
      protocol,
      collab,
      sink.clone(),
    );

    Self {
      object,
      origin,
      sink,
      stream,
      protocol: cloned_protocol,
    }
  }

  pub fn notify(&self, awareness: &Awareness) {
    if let Some(payload) = doc_init_state(awareness, &self.protocol) {
      self.sink.queue_msg(|msg_id| {
        ClientCollabInit::new(
          self.origin.clone(),
          self.object.object_id.clone(),
          self.object.workspace_id.clone(),
          msg_id,
          payload,
        )
        .into()
      });
    } else {
      self.sink.notify();
    }
  }

  pub fn clear(&self) {
    self.sink.remove_all_pending_msgs();
  }
}

fn doc_init_state<P: CollabSyncProtocol>(awareness: &Awareness, protocol: &P) -> Option<Vec<u8>> {
  let payload = {
    let mut encoder = EncoderV1::new();
    protocol.start(awareness, &mut encoder).ok()?;
    encoder.to_vec()
  };
  if payload.is_empty() {
    None
  } else {
    Some(payload)
  }
}

impl<Sink, Stream> Deref for SyncQueue<Sink, Stream> {
  type Target = Arc<CollabSink<Sink, CollabMessage>>;

  fn deref(&self) -> &Self::Target {
    &self.sink
  }
}

/// Use to continuously receive updates from remote.
struct SyncStream<Sink, Stream> {
  #[allow(dead_code)]
  weak_collab: Weak<MutexCollab>,
  #[allow(dead_code)]
  runner: JoinHandle<Result<(), SyncError>>,
  phantom_sink: PhantomData<Sink>,
  phantom_stream: PhantomData<Stream>,
}

impl<E, Sink, Stream> SyncStream<Sink, Stream>
where
  E: std::error::Error + Send + Sync + 'static,
  Sink: SinkExt<CollabMessage, Error = E> + Send + Sync + Unpin + 'static,
  Stream: StreamExt<Item = Result<CollabMessage, E>> + Send + Sync + Unpin + 'static,
{
  pub fn new<P>(
    origin: CollabOrigin,
    object_id: String,
    stream: Stream,
    protocol: P,
    weak_collab: Weak<MutexCollab>,
    sink: Arc<CollabSink<Sink, CollabMessage>>,
  ) -> Self
  where
    P: CollabSyncProtocol + Send + Sync + 'static,
  {
    let cloned_weak_collab = weak_collab.clone();
    let weak_sink = Arc::downgrade(&sink);
    let runner = spawn(SyncStream::<Sink, Stream>::spawn_doc_stream::<P>(
      origin,
      object_id,
      stream,
      cloned_weak_collab,
      weak_sink,
      protocol,
    ));
    Self {
      weak_collab,
      runner,
      phantom_sink: Default::default(),
      phantom_stream: Default::default(),
    }
  }

  // Spawn the stream that continuously reads the doc's updates from remote.
  async fn spawn_doc_stream<P>(
    origin: CollabOrigin,
    object_id: String,
    mut stream: Stream,
    weak_collab: Weak<MutexCollab>,
    weak_sink: Weak<CollabSink<Sink, CollabMessage>>,
    protocol: P,
  ) -> Result<(), SyncError>
  where
    P: CollabSyncProtocol + Send + Sync + 'static,
  {
    while let Some(input) = stream.next().await {
      match input {
        Ok(msg) => match (weak_collab.upgrade(), weak_sink.upgrade()) {
          (Some(awareness), Some(sink)) => {
            SyncStream::<Sink, Stream>::process_message::<P>(
              &origin, &object_id, &protocol, &awareness, &sink, msg,
            )
            .await?
          },
          _ => {
            tracing::trace!("ClientSync is dropped. Stopping receive incoming changes.");
            return Ok(());
          },
        },
        Err(e) => {
          // If the client has disconnected, the stream will return an error, So stop receiving
          // messages if the client has disconnected.
          return Err(SyncError::Internal(Box::new(e)));
        },
      }
    }
    Ok(())
  }

  /// Continuously handle messages from the remote doc
  async fn process_message<P>(
    origin: &CollabOrigin,
    object_id: &str,
    protocol: &P,
    collab: &Arc<MutexCollab>,
    sink: &Arc<CollabSink<Sink, CollabMessage>>,
    msg: CollabMessage,
  ) -> Result<(), SyncError>
  where
    P: CollabSyncProtocol + Send + Sync + 'static,
  {
    match msg {
      CollabMessage::ServerAck(ack) => {
        if let Some(payload) = &ack.payload {
          let mut decoder = DecoderV1::from(payload.as_ref());
          if let Ok(msg) = Message::decode(&mut decoder) {
            if let Some(resp_msg) = handle_msg(&Some(origin), protocol, collab, msg).await? {
              let payload = resp_msg.encode_v1();
              let object_id = object_id.to_string();
              sink.queue_msg(|msg_id| {
                CollabServerSync::new(origin.clone(), object_id, payload, msg_id).into()
              });
            }
          }
        }

        let msg_id = ack.msg_id;
        tracing::trace!(
          "[🙂Client {}]: {}",
          origin
            .client_user_id()
            .map(|uid| uid.to_string())
            .unwrap_or_default(),
          CollabMessage::ServerAck(ack)
        );
        sink.ack_msg(msg_id).await;
        Ok(())
      },
      _ => {
        let payload = msg.into_payload();
        if payload.is_empty() {
          return Ok(());
        }

        let mut decoder = DecoderV1::new(Cursor::new(&payload));
        let reader = MessageReader::new(&mut decoder);
        for msg in reader {
          let msg = msg?;
          if let Some(resp) = handle_msg(&Some(origin), protocol, collab, msg).await? {
            let payload = resp.encode_v1();
            let object_id = object_id.to_string();
            sink.queue_msg(|msg_id| {
              ClientCollabUpdate::new(origin.clone(), object_id, msg_id, payload).into()
            });
          }
        }
        Ok(())
      },
    }
  }
}
