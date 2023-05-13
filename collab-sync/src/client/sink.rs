use std::fmt::Display;
use std::marker::PhantomData;

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Weak};
use std::time::Duration;

use crate::client::pending_msg::{PendingMsgQueue, TaskState};
use crate::error::SyncError;
use futures_util::SinkExt;
use tokio::sync::{oneshot, watch, Mutex};

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
  pub(crate) fn notify(&self) {
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
