use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::ops::{Deref, DerefMut};
use tokio::sync::oneshot;

use crate::msg::CollabMessage;

pub(crate) struct PendingMsgQueue {
  queue: BinaryHeap<PendingMessage>,
}

impl PendingMsgQueue {
  pub(crate) fn new() -> Self {
    Self {
      queue: Default::default(),
    }
  }

  pub(crate) fn push_msg(&mut self, msg_id: u32, msg: CollabMessage) {
    self.queue.push(PendingMessage::new(msg, msg_id));
  }
}

impl Deref for PendingMsgQueue {
  type Target = BinaryHeap<PendingMessage>;

  fn deref(&self) -> &Self::Target {
    &self.queue
  }
}

impl DerefMut for PendingMsgQueue {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.queue
  }
}

#[derive(Debug)]
pub(crate) struct PendingMessage {
  msg: CollabMessage,
  msg_id: u32,
  state: TaskState,
  tx: Option<oneshot::Sender<u32>>,
}

impl PendingMessage {
  pub fn new(msg: CollabMessage, msg_id: u32) -> Self {
    Self {
      msg,
      msg_id,
      state: TaskState::Pending,
      tx: None,
    }
  }

  pub fn msg(&self) -> CollabMessage {
    self.msg.clone()
  }

  pub fn state(&self) -> &TaskState {
    &self.state
  }

  pub fn set_state(&mut self, new_state: TaskState) {
    self.state = new_state;

    if self.state.is_done() && self.tx.is_some() {
      self.tx.take().map(|tx| tx.send(self.msg_id));
    }
  }

  pub fn set_ret(&mut self, tx: oneshot::Sender<u32>) {
    self.tx = Some(tx);
  }

  pub fn msg_id(&self) -> u32 {
    self.msg_id
  }
}
impl Eq for PendingMessage {}

impl PartialEq for PendingMessage {
  fn eq(&self, other: &Self) -> bool {
    self.msg.msg_id() == other.msg.msg_id()
  }
}

impl PartialOrd for PendingMessage {
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
    Some(self.cmp(other))
  }
}

impl Ord for PendingMessage {
  fn cmp(&self, other: &Self) -> Ordering {
    match (&self.msg, &other.msg) {
      (CollabMessage::ClientInit { .. }, CollabMessage::ClientInit { .. }) => Ordering::Equal,
      (CollabMessage::ClientInit { .. }, _) => Ordering::Greater,
      _ => self.msg_id.cmp(&other.msg_id).reverse(),
    }
  }
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub enum TaskState {
  Pending,
  Processing,
  Done,
  Timeout,
}

impl TaskState {
  pub fn is_done(&self) -> bool {
    matches!(self, TaskState::Done)
  }
  pub fn is_processing(&self) -> bool {
    matches!(self, TaskState::Processing)
  }
  pub fn is_timeout(&self) -> bool {
    matches!(self, TaskState::Timeout)
  }
}
