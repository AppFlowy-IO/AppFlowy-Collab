use std::marker::PhantomData;
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
use tokio::time::interval;

use y_sync::sync::MessageReader;
use yrs::updates::decoder::DecoderV1;
use yrs::updates::encoder::Encode;

use crate::client::pending_msg::{PendingMsgQueue, TaskState};
use crate::error::SyncError;
use crate::msg::{ClientUpdateMessage, CollabMessage};
use crate::protocol::{handle_msg, CollabSyncProtocol, DefaultProtocol};

pub struct SyncQueue<Sink, Stream> {
  scheduler: Arc<Mutex<SinkTaskScheduler<Sink>>>,
  stream: SyncStream<Sink, Stream>,
  msg_id_counter: Arc<MsgIdCounter>,
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
    _stream: Stream,
    awareness: Arc<MutexCollabAwareness>,
  ) -> Self {
    let sender = Arc::new(Mutex::new(sink));
    let msg_id_counter = Arc::new(MsgIdCounter::new());
    let scheduler = Arc::new(Mutex::new(SinkTaskScheduler::new(
      sender.clone(),
      msg_id_counter.clone(),
      Duration::from_secs(5),
    )));
    spawn(TaskRunner::run(scheduler.clone()));
    let stream = SyncStream::new(
      origin,
      object_id.to_string(),
      _stream,
      DefaultProtocol,
      awareness,
      scheduler.clone(),
    );

    Self {
      scheduler,
      stream,
      msg_id_counter,
    }
  }
}

struct SyncStream<Sink, Stream> {
  origin: CollabOrigin,
  awareness: Arc<MutexCollabAwareness>,

  runner: JoinHandle<Result<(), SyncError>>,
  phantom_sink: PhantomData<Sink>,
  phantom_stream: PhantomData<Stream>,
}

impl<E, Sink, Stream> SyncStream<Sink, Stream>
where
  E: Into<SyncError> + Send + Sync,
  Sink: SinkExt<CollabMessage, Error = E> + Send + Sync + Unpin + 'static,
  Stream: StreamExt<Item = Result<CollabMessage, E>> + Send + Sync + Unpin + 'static,
{
  pub fn new<P>(
    origin: CollabOrigin,
    object_id: String,
    stream: Stream,
    protocol: P,
    awareness: Arc<MutexCollabAwareness>,
    scheduler: Arc<Mutex<SinkTaskScheduler<Sink>>>,
  ) -> Self
  where
    P: CollabSyncProtocol + Send + Sync + 'static,
  {
    let weak_awareness = Arc::downgrade(&awareness);
    let weak_scheduler = Arc::downgrade(&scheduler);
    let runner: JoinHandle<Result<(), SyncError>> = spawn(async move {
      SyncStream::<Sink, Stream>::spawn_doc_stream::<P>(
        object_id,
        stream,
        weak_awareness,
        weak_scheduler,
        protocol,
      )
      .await?;

      Ok(())
    });
    Self {
      origin,
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
    weak_scheduler: Weak<Mutex<SinkTaskScheduler<Sink>>>,
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
    scheduler: &Arc<Mutex<SinkTaskScheduler<Sink>>>,
    msg: CollabMessage,
  ) -> Result<(), SyncError>
  where
    P: CollabSyncProtocol + Send + Sync + 'static,
  {
    let origin = msg.origin();
    match msg {
      CollabMessage::ServerAck(ack) => {
        scheduler.lock().await.ack_msg(ack.msg_id);
        Ok(())
      },
      _ => {
        if msg.payload().is_some() {
          return Ok(());
        }
        let payload = msg.into_payload();
        let mut decoder = DecoderV1::new(Cursor::new(&payload));
        let reader = MessageReader::new(&mut decoder);
        for msg in reader {
          let msg = msg?;
          if let Some(resp) = handle_msg(&origin, protocol, awareness, msg).await? {
            let payload = resp.encode_v1();
            let object_id = object_id.to_string();
            let origin = origin.clone();
            scheduler.lock().await.sync_msg(|msg_id| {
              ClientUpdateMessage::new(origin, object_id, msg_id, payload).into()
            });
          }
        }
        Ok(())
      },
    }
  }
}

struct SinkTaskScheduler<Sink> {
  sender: Arc<Mutex<Sink>>,
  pending_msgs: Arc<Mutex<PendingMsgQueue>>,
  msg_id_counter: Arc<MsgIdCounter>,
  notifier: watch::Sender<bool>,
  notifier_rx: Option<watch::Receiver<bool>>,
  timeout: Duration,
}

impl<E, Sink> SinkTaskScheduler<Sink>
where
  E: Into<SyncError> + Send + Sync,
  Sink: SinkExt<CollabMessage, Error = E> + Send + Sync + Unpin + 'static,
{
  fn new(sender: Arc<Mutex<Sink>>, msg_id_counter: Arc<MsgIdCounter>, timeout: Duration) -> Self {
    let pending_msgs = Arc::new(Mutex::new(PendingMsgQueue::new()));
    let (notifier, notifier_rx) = watch::channel(false);
    Self {
      sender,
      pending_msgs,
      msg_id_counter,
      notifier,
      notifier_rx: Some(notifier_rx),
      timeout,
    }
  }

  pub async fn sync_msg(&self, f: impl FnOnce(u32) -> CollabMessage) -> Result<(), SyncError> {
    let mut pending_msgs = self.pending_msgs.lock().await;
    let msg_id = self.msg_id_counter.next();
    let msg = f(msg_id);
    pending_msgs.push_msg(msg_id, msg);
    drop(pending_msgs);

    self.notify();
    Ok(())
  }

  pub async fn ack_msg(&self, msg_id: u32) {
    self
      .pending_msgs
      .lock()
      .await
      .peek_mut()
      .map(|mut pending_msg| {
        if pending_msg.msg_id() == msg_id {
          pending_msg.set_state(TaskState::Done);
        }
      });
  }

  async fn process_next_msg(&self) -> Result<(), SyncError> {
    let mut pending_msgs = self.pending_msgs.lock().await;
    let pending_msg = pending_msgs.pop();
    match pending_msg {
      Some(mut pending_msg) => {
        // If the task is finished, remove the message from the queue.
        // And schedule the next task.
        if pending_msg.state().is_done() {
          self.notify();
          return Ok(());
        }

        // Update the pending message's msg_id and send the message.
        let (tx, rx) = oneshot::channel();
        pending_msg.set_state(TaskState::Processing);
        pending_msg.set_ret(tx);

        let collab_msg = pending_msg.msg();
        pending_msgs.push(pending_msg);
        drop(pending_msgs);

        match tokio::time::timeout(self.timeout, rx).await {
          Ok(_) => self.notify(),
          Err(_) => {
            // If the task is timeout, just resend the message.
            self.notify();
          },
        }

        let mut sender = self.sender.lock().await;
        sender.send(collab_msg).await.map_err(|e| e.into())
      },
      None => Ok(()),
    }
  }
  fn notify(&self) {
    let _ = self.notifier.send(false);
  }

  fn stop(&self) {
    let _ = self.notifier.send(true);
  }
}

pub struct TaskRunner();

impl TaskRunner {
  async fn run<E, Sink>(scheduler: Arc<Mutex<SinkTaskScheduler<Sink>>>)
  where
    E: Into<SyncError> + Send + Sync + 'static,
    Sink: SinkExt<CollabMessage, Error = E> + Send + Sync + Unpin + 'static,
  {
    scheduler.lock().await.notify();
    let debounce_duration = Duration::from_millis(300);
    let mut notifier = scheduler
      .lock()
      .await
      .notifier_rx
      .take()
      .expect("Only take once");
    loop {
      // stops the runner if the notifier was closed.
      if notifier.changed().await.is_err() {
        break;
      }

      // stops the runner if the value of notifier is `true`
      if *notifier.borrow() {
        break;
      }

      let mut interval = interval(debounce_duration);
      interval.tick().await;
      let _ = scheduler.lock().await.process_next_msg().await;
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
