use std::marker::PhantomData;
use std::ops::Deref;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Weak};
use std::time::Duration;

use collab::core::collab::CollabOrigin;
use collab::core::collab_awareness::MutexCollabAwareness;
use futures_util::{SinkExt, StreamExt};
use lib0::decoding::Cursor;
use tokio::spawn;
use tokio::sync::{oneshot, watch, Mutex};
use tokio::task::JoinHandle;

use y_sync::sync::MessageReader;
use yrs::updates::decoder::DecoderV1;
use yrs::updates::encoder::{Encode, Encoder, EncoderV1};

use crate::client::pending_msg::{PendingMsgQueue, TaskState};
use crate::error::SyncError;
use crate::msg::{ClientInitMessage, ClientUpdateMessage, CollabMessage};
use crate::protocol::{handle_msg, CollabSyncProtocol, DefaultProtocol};

pub const SYNC_TIMEOUT: u64 = 2;

pub struct SyncQueue<Sink, Stream> {
  scheduler: Arc<SinkScheduler<Sink>>,
  #[allow(dead_code)]
  stream: SyncStream<Sink, Stream>,
}

impl<E, Sink, Stream> SyncQueue<Sink, Stream>
where
  E: Into<SyncError> + Send + Sync + 'static,
  Sink: SinkExt<CollabMessage, Error = E> + Send + Sync + Unpin + 'static,
  Stream: StreamExt<Item = Result<CollabMessage, E>> + Send + Sync + Unpin + 'static,
{
  pub fn new(
    object_id: &str,
    origin: CollabOrigin,
    sink: Sink,
    stream: Stream,
    awareness: Arc<MutexCollabAwareness>,
  ) -> Self {
    let protocol = DefaultProtocol;
    let sender = Arc::new(Mutex::new(sink));
    let msg_id_counter = Arc::new(MsgIdCounter::new());
    let msg_id = msg_id_counter.next();
    let mut pending_msgs = PendingMsgQueue::new();
    if let Some(payload) = doc_init_state(&awareness, &protocol) {
      let msg = ClientInitMessage::new(origin, object_id.to_string(), msg_id, payload);
      pending_msgs.push_msg(msg_id, msg.into());
    }
    let (notifier, notifier_rx) = watch::channel(false);
    let scheduler = Arc::new(SinkScheduler::new(
      sender,
      msg_id_counter,
      pending_msgs,
      notifier,
      Duration::from_secs(SYNC_TIMEOUT),
    ));
    spawn(TaskRunner::run(scheduler.clone(), notifier_rx));

    let stream = SyncStream::new(
      object_id.to_string(),
      stream,
      protocol,
      awareness,
      scheduler.clone(),
    );

    Self { scheduler, stream }
  }
}

fn doc_init_state<P: CollabSyncProtocol>(
  awareness: &Arc<MutexCollabAwareness>,
  protocol: &P,
) -> Option<Vec<u8>> {
  let payload = {
    let mut encoder = EncoderV1::new();
    let awareness = awareness.lock();
    protocol.start(&awareness, &mut encoder).ok()?;
    encoder.to_vec()
  };
  if payload.is_empty() {
    None
  } else {
    Some(payload)
  }
}

impl<Sink, Stream> Deref for SyncQueue<Sink, Stream> {
  type Target = Arc<SinkScheduler<Sink>>;

  fn deref(&self) -> &Self::Target {
    &self.scheduler
  }
}

struct SyncStream<Sink, Stream> {
  #[allow(dead_code)]
  awareness: Arc<MutexCollabAwareness>,
  #[allow(dead_code)]
  runner: JoinHandle<Result<(), SyncError>>,
  phantom_sink: PhantomData<Sink>,
  phantom_stream: PhantomData<Stream>,
}

impl<E, Sink, Stream> SyncStream<Sink, Stream>
where
  E: Into<SyncError> + Send + Sync + 'static,
  Sink: SinkExt<CollabMessage, Error = E> + Send + Sync + Unpin + 'static,
  Stream: StreamExt<Item = Result<CollabMessage, E>> + Send + Sync + Unpin + 'static,
{
  pub fn new<P>(
    object_id: String,
    stream: Stream,
    protocol: P,
    awareness: Arc<MutexCollabAwareness>,
    scheduler: Arc<SinkScheduler<Sink>>,
  ) -> Self
  where
    P: CollabSyncProtocol + Send + Sync + 'static,
  {
    let weak_awareness = Arc::downgrade(&awareness);
    let weak_scheduler = Arc::downgrade(&scheduler);
    let runner = spawn(SyncStream::<Sink, Stream>::spawn_doc_stream::<P>(
      object_id,
      stream,
      weak_awareness,
      weak_scheduler,
      protocol,
    ));
    Self {
      awareness,
      runner,
      phantom_sink: Default::default(),
      phantom_stream: Default::default(),
    }
  }

  // Spawn the stream that continuously reads the doc's updates.
  async fn spawn_doc_stream<P>(
    object_id: String,
    mut stream: Stream,
    weak_awareness: Weak<MutexCollabAwareness>,
    weak_scheduler: Weak<SinkScheduler<Sink>>,
    protocol: P,
  ) -> Result<(), SyncError>
  where
    P: CollabSyncProtocol + Send + Sync + 'static,
  {
    while let Some(input) = stream.next().await {
      match input {
        Ok(msg) => match (weak_awareness.upgrade(), weak_scheduler.upgrade()) {
          (Some(awareness), Some(scheduler)) => {
            SyncStream::<Sink, Stream>::process_message::<P>(
              &object_id, &protocol, &awareness, &scheduler, msg,
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
          return Err(e.into());
        },
      }
    }
    Ok(())
  }

  /// Continuously handle messages from the remote doc
  async fn process_message<P>(
    object_id: &str,
    protocol: &P,
    awareness: &Arc<MutexCollabAwareness>,
    scheduler: &Arc<SinkScheduler<Sink>>,
    msg: CollabMessage,
  ) -> Result<(), SyncError>
  where
    P: CollabSyncProtocol + Send + Sync + 'static,
  {
    let origin = msg.origin();
    match msg {
      CollabMessage::ServerAck(ack) => {
        let msg_id = ack.msg_id;
        tracing::trace!("[🦀Client]: {}", CollabMessage::ServerAck(ack));
        scheduler.ack_msg(msg_id).await;

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
          if let Some(resp) = handle_msg(&origin, protocol, awareness, msg).await? {
            let payload = resp.encode_v1();
            let object_id = object_id.to_string();
            let origin = origin.clone();
            scheduler
              .sync_msg(|msg_id| {
                ClientUpdateMessage::new(origin, object_id, msg_id, payload).into()
              })
              .await;
          }
        }
        Ok(())
      },
    }
  }
}

pub struct SinkScheduler<Sink> {
  sender: Arc<Mutex<Sink>>,
  pending_msgs: Arc<Mutex<PendingMsgQueue>>,
  msg_id_counter: Arc<MsgIdCounter>,
  notifier: watch::Sender<bool>,
  timeout: Duration,
}

impl<E, Sink> SinkScheduler<Sink>
where
  E: Into<SyncError> + Send + Sync,
  Sink: SinkExt<CollabMessage, Error = E> + Send + Sync + Unpin + 'static,
{
  fn new(
    sender: Arc<Mutex<Sink>>,
    msg_id_counter: Arc<MsgIdCounter>,
    pending_msgs: PendingMsgQueue,
    notifier: watch::Sender<bool>,
    timeout: Duration,
  ) -> Self {
    let pending_msgs = Arc::new(Mutex::new(pending_msgs));

    Self {
      sender,
      pending_msgs,
      msg_id_counter,
      notifier,
      timeout,
    }
  }

  pub async fn sync_msg(&self, f: impl FnOnce(u32) -> CollabMessage) {
    let mut pending_msgs = self.pending_msgs.lock().await;
    let msg_id = self.msg_id_counter.next();
    let msg = f(msg_id);
    pending_msgs.push_msg(msg_id, msg);
    drop(pending_msgs);

    self.notify();
  }

  /// Notify the scheduler to process the next message and mark the current message as done.
  pub async fn ack_msg(&self, msg_id: u32) {
    if let Some(mut pending_msg) = self.pending_msgs.lock().await.peek_mut() {
      if pending_msg.msg_id() == msg_id {
        pending_msg.set_state(TaskState::Done);
      }
    }
    self.notify();
  }

  async fn process_next_msg(&self) -> Result<(), SyncError> {
    let mut pending_msgs = self.pending_msgs.lock().await;
    let pending_msg = pending_msgs.pop();
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
        pending_msgs.push(pending_msg);
        drop(pending_msgs);

        let mut sender = self.sender.lock().await;
        tracing::trace!("[🦀Client]: {}", collab_msg);
        sender.send(collab_msg).await.map_err(|e| e.into())?;

        // Wait for the message to be acked.
        // If the message is not acked within the timeout, resend the message.
        match tokio::time::timeout(self.timeout, rx).await {
          Ok(_) => self.notify(),
          Err(_) => {
            if let Some(mut pending_msg) = self.pending_msgs.lock().await.peek_mut() {
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

  /// Notify the scheduler to process the next message.
  fn notify(&self) {
    let _ = self.notifier.send(false);
  }

  /// Stop the scheduler.
  #[allow(dead_code)]
  fn stop(&self) {
    let _ = self.notifier.send(true);
  }
}

pub struct TaskRunner();

impl TaskRunner {
  async fn run<E, Sink>(scheduler: Arc<SinkScheduler<Sink>>, mut notifier: watch::Receiver<bool>)
  where
    E: Into<SyncError> + Send + Sync + 'static,
    Sink: SinkExt<CollabMessage, Error = E> + Send + Sync + Unpin + 'static,
  {
    scheduler.notify();
    loop {
      // stops the runner if the notifier was closed.
      if notifier.changed().await.is_err() {
        break;
      }

      // stops the runner if the value of notifier is `true`
      if *notifier.borrow() {
        break;
      }

      let _ = scheduler.process_next_msg().await;
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
