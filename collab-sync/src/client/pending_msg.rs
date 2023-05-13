use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::ops::{Deref, DerefMut};

use tokio::sync::oneshot;

pub(crate) struct PendingMsgQueue<Msg> {
  queue: BinaryHeap<PendingMessage<Msg>>,
}

impl<Msg> PendingMsgQueue<Msg>
where
  Msg: Ord + Clone,
{
  pub(crate) fn new() -> Self {
    Self {
      queue: Default::default(),
    }
  }

  pub(crate) fn push_msg(&mut self, msg_id: u32, msg: Msg) {
    self.queue.push(PendingMessage::new(msg, msg_id));
  }
}

impl<Msg> Deref for PendingMsgQueue<Msg>
where
  Msg: Ord,
{
  type Target = BinaryHeap<PendingMessage<Msg>>;

  fn deref(&self) -> &Self::Target {
    &self.queue
  }
}

impl<Msg> DerefMut for PendingMsgQueue<Msg>
where
  Msg: Ord,
{
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.queue
  }
}

#[derive(Debug)]
pub(crate) struct PendingMessage<Msg> {
  msg: Msg,
  msg_id: u32,
  state: TaskState,
  tx: Option<oneshot::Sender<u32>>,
}

impl<Msg> PendingMessage<Msg>
where
  Msg: Clone,
{
  pub fn new(msg: Msg, msg_id: u32) -> Self {
    Self {
      msg,
      msg_id,
      state: TaskState::Pending,
      tx: None,
    }
  }

  pub fn msg(&self) -> Msg {
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

impl<Msg> Eq for PendingMessage<Msg> where Msg: Eq {}

impl<Msg> PartialEq for PendingMessage<Msg>
where
  Msg: PartialEq,
{
  fn eq(&self, other: &Self) -> bool {
    self.msg == other.msg
  }
}

impl<Msg> PartialOrd for PendingMessage<Msg>
where
  Msg: PartialOrd + Ord,
{
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
    Some(self.cmp(other))
  }
}

impl<Msg> Ord for PendingMessage<Msg>
where
  Msg: Ord,
{
  fn cmp(&self, other: &Self) -> Ordering {
    self.msg.cmp(&other.msg)
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
}
