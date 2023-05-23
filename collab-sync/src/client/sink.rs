use std::fmt::Display;
use std::marker::PhantomData;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Weak};
use std::time::Duration;

use futures_util::SinkExt;
use tokio::spawn;
use tokio::sync::{mpsc, oneshot, watch, Mutex};
use tokio::time::{interval, Instant, Interval};

use crate::client::pending_msg::{PendingMsgQueue, TaskState};
use crate::client::sync::DEFAULT_SYNC_TIMEOUT;
use crate::error::SyncError;

pub trait SinkMessage: Clone + Send + Sync + 'static + Ord + Display {
  /// Returns the length of the message in bytes.
  fn length(&self) -> usize;
  /// Returns true if the message can be merged with other messages.
  /// Check the implementation of `queue_or_merge_msg` for more details.
  fn can_merge(&self) -> bool;
}

/// Use to sync the [Msg] to the remote.
pub struct CollabSink<Sink, Msg> {
  /// The [Sink] is used to send the messages to the remote. It might be a websocket sink or
  /// other sink that implements the [SinkExt] trait.
  sender: Arc<Mutex<Sink>>,

  /// The [PendingMsgQueue] is used to queue the messages that are waiting to be sent to the
  /// remote. It will merge the messages if possible.
  pending_msgs: Arc<parking_lot::Mutex<PendingMsgQueue<Msg>>>,
  msg_id_counter: Arc<dyn MsgIdCounter>,

  /// The [watch::Sender] is used to notify the [CollabSinkRunner] to process the pending messages.
  /// Sending `false` will stop the [CollabSinkRunner].
  notifier: Arc<watch::Sender<bool>>,
  config: SinkConfig,

  /// Stop the [IntervalRunner] if the sink strategy is [SinkStrategy::FixInterval].
  #[allow(dead_code)]
  interval_runner_stop_tx: Option<mpsc::Sender<()>>,

  /// Used to calculate the time interval between two messages. Only used when the sink strategy
  /// is [SinkStrategy::FixInterval].
  instant: Mutex<Instant>,
}

impl<E, Sink, Msg> CollabSink<Sink, Msg>
where
  E: std::error::Error + Send + Sync + 'static,
  Sink: SinkExt<Msg, Error = E> + Send + Sync + Unpin + 'static,
  Msg: SinkMessage,
{
  pub fn new<C>(
    sink: Sink,
    notifier: watch::Sender<bool>,
    msg_id_counter: C,
    config: SinkConfig,
  ) -> Self
  where
    C: MsgIdCounter,
  {
    let notifier = Arc::new(notifier);
    let sender = Arc::new(Mutex::new(sink));
    let pending_msgs = PendingMsgQueue::new();
    let pending_msgs = Arc::new(parking_lot::Mutex::new(pending_msgs));
    let msg_id_counter = Arc::new(msg_id_counter);
    //
    let instant = Mutex::new(Instant::now());
    let mut interval_runner_stop_tx = None;
    if let SinkStrategy::FixInterval(duration) = &config.strategy {
      let weak_notifier = Arc::downgrade(&notifier);
      let (tx, rx) = mpsc::channel(1);
      interval_runner_stop_tx = Some(tx);
      spawn(IntervalRunner::new(*duration).run(weak_notifier, rx));
    }

    Self {
      sender,
      pending_msgs,
      msg_id_counter,
      notifier,
      config,
      instant,
      interval_runner_stop_tx,
    }
  }

  /// Put the message into the queue and notify the sink to process the next message
  pub fn queue_msg(&self, f: impl FnOnce(MsgId) -> Msg) {
    {
      let mut pending_msgs = self.pending_msgs.lock();
      let msg_id = self.msg_id_counter.next();
      let msg = f(msg_id);
      pending_msgs.push_msg(msg_id, msg);
      drop(pending_msgs);
    }

    self.notify();
  }

  /// Queue the message or merge it with the previous message if possible.
  pub fn queue_or_merge_msg(
    &self,
    merge: impl FnOnce(&mut Msg) -> Result<(), SyncError>,
    or_else: impl FnOnce(MsgId) -> Msg,
  ) {
    {
      let mut pending_msgs = self.pending_msgs.lock();
      if let Some(mut prev) = pending_msgs.peek_mut() {
        // Only merge the message if the previous message is pending and can be merged.
        // Otherwise, just queue the new message.
        if prev.state().is_pending() {
          let prev_msg = prev.get_mut_msg();
          if prev_msg.can_merge() && merge(prev_msg).is_ok() {
            tracing::trace!("Did merge new message, len: {}", prev_msg.length());
            return;
          }
        }
      }

      let msg_id = self.msg_id_counter.next();
      let msg = or_else(msg_id);
      pending_msgs.push_msg(msg_id, msg);
      drop(pending_msgs);
    }
    self.notify();
  }

  /// Notify the sink to process the next message and mark the current message as done.
  pub async fn ack_msg(&self, msg_id: MsgId) {
    if let Some(mut pending_msg) = self.pending_msgs.lock().peek_mut() {
      if pending_msg.msg_id() == msg_id {
        pending_msg.set_state(TaskState::Done);
      }
    }
    self.notify();
  }

  async fn process_next_msg(&self) -> Result<(), SyncError> {
    if let SinkStrategy::FixInterval(duration) = &self.config.strategy {
      let elapsed = self.instant.lock().await.elapsed();
      // tracing::trace!(
      //   "elapsed interval: {:?}, fix interval: {:?}",
      //   elapsed,
      //   duration
      // );
      if elapsed < *duration {
        return Ok(());
      }
    }

    let pending_msg = match self.pending_msgs.try_lock() {
      None => {
        // If acquire the lock failed, try to notify again after 100ms
        let weak_notifier = Arc::downgrade(&self.notifier);
        spawn(async move {
          interval(Duration::from_millis(100)).tick().await;
          if let Some(notifier) = weak_notifier.upgrade() {
            let _ = notifier.send(false);
          }
        });
        None
      },
      Some(mut pending_msg) => pending_msg.pop(),
    };

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
        match tokio::time::timeout(self.config.timeout, rx).await {
          Ok(_) => self.notify(),
          Err(_) => {
            if let Some(mut pending_msg) = self.pending_msgs.lock().peek_mut() {
              pending_msg.set_state(TaskState::Timeout);
            }
            self.notify();
          },
        }

        if self.config.strategy.is_fix_interval() {
          *self.instant.lock().await = Instant::now();
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

pub struct CollabSinkRunner<Msg>(PhantomData<Msg>);
impl<Msg> CollabSinkRunner<Msg> {
  /// The runner will stop if the [CollabSink] was dropped or the notifier was closed.
  pub async fn run<E, Sink>(
    weak_sink: Weak<CollabSink<Sink, Msg>>,
    mut notifier: watch::Receiver<bool>,
  ) where
    E: std::error::Error + Send + Sync + 'static,
    Sink: SinkExt<Msg, Error = E> + Send + Sync + Unpin + 'static,
    Msg: SinkMessage,
  {
    weak_sink.upgrade().unwrap().notify();
    loop {
      // stops the runner if the notifier was closed.
      if notifier.changed().await.is_err() {
        break;
      }

      // stops the runner if the value of notifier is `true`
      if *notifier.borrow() {
        break;
      }

      if let Some(sync_sink) = weak_sink.upgrade() {
        let _ = sync_sink.process_next_msg().await;
      } else {
        break;
      }
    }
  }
}

pub struct SinkConfig {
  /// `timeout` is the time to wait for the remote to ack the message. If the remote
  /// does not ack the message in time, the message will be sent again.
  pub timeout: Duration,
  /// `mergeable` indicates whether the messages are mergeable. If the messages are
  /// mergeable, the sink will try to merge the messages before sending them.
  pub mergeable: bool,
  /// `max_zip_size` is the maximum size of the messages to be merged.
  pub max_merge_size: usize,
  /// `strategy` is the strategy to send the messages.
  pub strategy: SinkStrategy,
}

impl SinkConfig {
  pub fn new() -> Self {
    Self::default()
  }
  pub fn with_timeout(mut self, secs: u64) -> Self {
    let timeout_duration = Duration::from_secs(secs);
    if let SinkStrategy::FixInterval(duration) = self.strategy {
      if timeout_duration < duration {
        tracing::warn!("The timeout duration should greater than the fix interval duration");
      }
    }
    self.timeout = timeout_duration;
    self
  }

  /// `mergeable` indicates whether the messages are mergeable. If the messages are
  /// mergeable, the sink will try to merge the messages before sending them.
  pub fn with_mergeable(mut self, mergeable: bool) -> Self {
    self.mergeable = mergeable;
    self
  }

  /// `max_zip_size` is the maximum size of the messages to be merged.
  pub fn with_max_merge_size(mut self, max_merge_size: usize) -> Self {
    self.max_merge_size = max_merge_size;
    self
  }

  pub fn with_strategy(mut self, strategy: SinkStrategy) -> Self {
    if let SinkStrategy::FixInterval(duration) = strategy {
      if self.timeout < duration {
        tracing::warn!("The timeout duration should greater than the fix interval duration");
      }
    }
    self.strategy = strategy;
    self
  }
}

impl Default for SinkConfig {
  fn default() -> Self {
    Self {
      timeout: Duration::from_secs(DEFAULT_SYNC_TIMEOUT),
      mergeable: false,
      max_merge_size: 1024,
      strategy: SinkStrategy::ASAP,
    }
  }
}

pub enum SinkStrategy {
  /// Send the message as soon as possible.
  ASAP,
  /// Send the message in a fixed interval.
  /// This can reduce the number of times the message is sent. Especially if using the AWS
  /// as the storage layer, the cost of sending the message is high. However, it may increase
  /// the latency of the message.
  FixInterval(Duration),
}

impl SinkStrategy {
  pub fn is_fix_interval(&self) -> bool {
    matches!(self, SinkStrategy::FixInterval(_))
  }
}

pub type MsgId = u64;

pub trait MsgIdCounter: Send + Sync + 'static {
  /// Get the next message id. The message id should be unique.
  fn next(&self) -> MsgId;
}

#[derive(Debug, Default)]
pub struct DefaultMsgIdCounter(Arc<AtomicU64>);
impl DefaultMsgIdCounter {
  pub fn new() -> Self {
    Self::default()
  }
}

impl MsgIdCounter for DefaultMsgIdCounter {
  fn next(&self) -> MsgId {
    self.0.fetch_add(1, Ordering::SeqCst)
  }
}

struct IntervalRunner {
  interval: Option<Interval>,
}

impl IntervalRunner {
  fn new(duration: Duration) -> Self {
    Self {
      interval: Some(tokio::time::interval(duration)),
    }
  }
}

impl IntervalRunner {
  pub async fn run(mut self, sender: Weak<watch::Sender<bool>>, mut stop_rx: mpsc::Receiver<()>) {
    let mut interval = self
      .interval
      .take()
      .expect("Interval should only take once");
    loop {
      tokio::select! {
        _ = stop_rx.recv() => {
            break;
        },
        _ = interval.tick() => {
          if let Some(sender) = sender.upgrade() {
            let _ = sender.send(false);
          } else {
            break;
          }
        }
      }
    }
  }
}
