use std::fmt::Display;
use std::marker::PhantomData;
use std::ops::Deref;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Weak};
use std::time::Duration;

use collab::core::collab::MutexCollab;
use collab::core::origin::CollabOrigin;
use futures_util::{SinkExt, StreamExt};
use lib0::decoding::Cursor;
use tokio::spawn;
use tokio::sync::{oneshot, watch, Mutex};
use tokio::task::JoinHandle;
use y_sync::awareness::Awareness;
use y_sync::sync::{Message, MessageReader};
use yrs::updates::decoder::{Decode, DecoderV1};
use yrs::updates::encoder::{Encode, Encoder, EncoderV1};

use crate::client::pending_msg::{PendingMsgQueue, TaskState};
use crate::error::SyncError;
use crate::msg::{CSClientInit, CSClientUpdate, CSServerSync, CollabMessage};
use crate::protocol::{handle_msg, CollabSyncProtocol, DefaultSyncProtocol};

pub const DEFAULT_SYNC_TIMEOUT: u64 = 2;

pub struct SyncQueue<Sink, Stream> {
  object_id: String,
  origin: CollabOrigin,
  /// The [SyncSink] is used to send the updates to the remote. It will send the current
  /// update periodically if the timeout is reached or it will send the next update if
  /// it receive previous ack from the remote.
  sink: Arc<SyncSink<Sink, CollabMessage>>,
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
    object_id: &str,
    origin: CollabOrigin,
    sink: Sink,
    stream: Stream,
    collab: Arc<MutexCollab>,
    timeout: u64,
  ) -> Self {
    let protocol = DefaultSyncProtocol;
    let (notifier, notifier_rx) = watch::channel(false);
    let sink = Arc::new(SyncSink::new(sink, notifier, Duration::from_secs(timeout)));
    spawn(TaskRunner::run(Arc::downgrade(&sink), notifier_rx));
    let cloned_protocol = protocol.clone();
    let object_id = object_id.to_string();
    let stream = SyncStream::new(
      origin.clone(),
      object_id.to_string(),
      stream,
      protocol,
      collab,
      sink.clone(),
    );

    Self {
      object_id,
      origin,
      sink,
      stream,
      protocol: cloned_protocol,
    }
  }

  pub fn notify(&self, awareness: &Awareness) {
    if let Some(payload) = doc_init_state(awareness, &self.protocol) {
      self.sink.queue_msg(|msg_id| {
        CSClientInit::new(self.origin.clone(), self.object_id.clone(), msg_id, payload).into()
      });
    } else {
      self.sink.notify();
    }
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
  type Target = Arc<SyncSink<Sink, CollabMessage>>;

  fn deref(&self) -> &Self::Target {
    &self.sink
  }
}

/// Use to continuously receive updates from remote.
struct SyncStream<Sink, Stream> {
  #[allow(dead_code)]
  collab: Arc<MutexCollab>,
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
    collab: Arc<MutexCollab>,
    sink: Arc<SyncSink<Sink, CollabMessage>>,
  ) -> Self
  where
    P: CollabSyncProtocol + Send + Sync + 'static,
  {
    let weak_collab = Arc::downgrade(&collab);
    let weak_sink = Arc::downgrade(&sink);
    let runner = spawn(SyncStream::<Sink, Stream>::spawn_doc_stream::<P>(
      origin,
      object_id,
      stream,
      weak_collab,
      weak_sink,
      protocol,
    ));
    Self {
      collab,
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
    weak_sink: Weak<SyncSink<Sink, CollabMessage>>,
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
    sink: &Arc<SyncSink<Sink, CollabMessage>>,
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
                CSServerSync::new(origin.clone(), object_id, payload, msg_id).into()
              });
            }
          }
        }

        let msg_id = ack.msg_id;
        tracing::trace!("[🦀Client]: {}", CollabMessage::ServerAck(ack));
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
              CSClientUpdate::new(origin.clone(), object_id, msg_id, payload).into()
            });
          }
        }
        Ok(())
      },
    }
  }
}

pub struct SyncSink<Sink, Msg> {
  sender: Arc<Mutex<Sink>>,
  pending_msgs: Arc<parking_lot::Mutex<PendingMsgQueue<Msg>>>,
  msg_id_counter: Arc<MsgIdCounter>,
  notifier: watch::Sender<bool>,
  timeout: Duration,
}

impl<E, Sink, Msg> SyncSink<Sink, Msg>
where
  E: std::error::Error + Send + Sync + 'static,
  Sink: SinkExt<Msg, Error = E> + Send + Sync + Unpin + 'static,
  Msg: Clone + Send + Sync + 'static + Ord + Display,
{
  pub fn new(sink: Sink, notifier: watch::Sender<bool>, timeout: Duration) -> Self {
    let sender = Arc::new(Mutex::new(sink));
    let pending_msgs = PendingMsgQueue::new();
    let msg_id_counter = Arc::new(MsgIdCounter::new());
    let pending_msgs = Arc::new(parking_lot::Mutex::new(pending_msgs));
    Self {
      sender,
      pending_msgs,
      msg_id_counter,
      notifier,
      timeout,
    }
  }

  pub fn queue_msg(&self, f: impl FnOnce(u32) -> Msg) {
    {
      let mut pending_msgs = self.pending_msgs.lock();
      let msg_id = self.msg_id_counter.next();
      let msg = f(msg_id);
      pending_msgs.push_msg(msg_id, msg);
      drop(pending_msgs);
    }

    self.notify();
  }

  /// Notify the sink to process the next message and mark the current message as done.
  pub async fn ack_msg(&self, msg_id: u32) {
    if let Some(mut pending_msg) = self.pending_msgs.lock().peek_mut() {
      if pending_msg.msg_id() == msg_id {
        pending_msg.set_state(TaskState::Done);
      }
    }
    self.notify();
  }

  async fn process_next_msg(&self) -> Result<(), SyncError> {
    let pending_msg = self.pending_msgs.lock().pop();
    match pending_msg {
      Some(mut pending_msg) => {
        if pending_msg.state().is_done() {
          // Notify to process the next pending message
          self.notify();
          return Ok(());
        }

        // Do nothing if the message is still processing.
        if pending_msg.state().is_processing() {
          return Ok(());
        }

        // Update the pending message's msg_id and send the message.
        let (tx, rx) = oneshot::channel();
        pending_msg.set_state(TaskState::Processing);
        pending_msg.set_ret(tx);

        // Push back the pending message to the queue.
        let collab_msg = pending_msg.msg();
        self.pending_msgs.lock().push(pending_msg);

        let mut sender = self.sender.lock().await;
        tracing::trace!("[🦀Client]: {}", collab_msg);
        sender
          .send(collab_msg)
          .await
          .map_err(|e| SyncError::Internal(Box::new(e)))?;

        // Wait for the message to be acked.
        // If the message is not acked within the timeout, resend the message.
        match tokio::time::timeout(self.timeout, rx).await {
          Ok(_) => self.notify(),
          Err(_) => {
            if let Some(mut pending_msg) = self.pending_msgs.lock().peek_mut() {
              pending_msg.set_state(TaskState::Timeout);
            }
            self.notify();
          },
        }
        Ok(())
      },
      None => Ok(()),
    }
  }

  /// Notify the sink to process the next message.
  fn notify(&self) {
    let _ = self.notifier.send(false);
  }

  /// Stop the sink.
  #[allow(dead_code)]
  fn stop(&self) {
    let _ = self.notifier.send(true);
  }
}

pub struct TaskRunner<Msg>(PhantomData<Msg>);

impl<Msg> TaskRunner<Msg> {
  /// The runner will stop if the [SyncSink] was dropped or the notifier was closed.
  pub async fn run<E, Sink>(
    sync_sink: Weak<SyncSink<Sink, Msg>>,
    mut notifier: watch::Receiver<bool>,
  ) where
    E: std::error::Error + Send + Sync + 'static,
    Sink: SinkExt<Msg, Error = E> + Send + Sync + Unpin + 'static,
    Msg: Clone + Send + Sync + 'static + Ord + Display,
  {
    sync_sink.upgrade().unwrap().notify();
    loop {
      // stops the runner if the notifier was closed.
      if notifier.changed().await.is_err() {
        break;
      }

      // stops the runner if the value of notifier is `true`
      if *notifier.borrow() {
        break;
      }

      if let Some(sync_sink) = sync_sink.upgrade() {
        let _ = sync_sink.process_next_msg().await;
      } else {
        break;
      }
    }
  }
}

struct MsgIdCounter(Arc<AtomicU32>);

impl MsgIdCounter {
  fn new() -> Self {
    Self(Arc::new(AtomicU32::new(0)))
  }

  fn next(&self) -> u32 {
    self.0.fetch_add(1, Ordering::SeqCst)
  }
}
