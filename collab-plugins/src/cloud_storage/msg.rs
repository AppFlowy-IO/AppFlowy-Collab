use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::fmt::Display;
use std::ops::{Deref, DerefMut};

use tokio::sync::oneshot;

pub type MsgId = u64;

#[allow(dead_code)]
pub trait CollabSinkMessage: Clone + Send + Sync + 'static + Ord + Display {
  fn object_id(&self) -> &str;
  /// Returns the length of the message in bytes.
  fn length(&self) -> usize;
  /// Returns true if the message can be merged with other messages.
  fn mergeable(&self) -> bool;

  fn merge(&mut self, other: &Self) -> bool;

  fn is_init_msg(&self) -> bool;

  /// Determine if the message can be deferred base on the current state of the sink.
  fn deferrable(&self) -> bool;
}
pub(crate) struct PendingMsgQueue<Msg> {
  queue: BinaryHeap<PendingMessage<Msg>>,
}

impl<Msg> PendingMsgQueue<Msg>
where
  Msg: Ord + Clone + Display,
{
  pub(crate) fn new() -> Self {
    Self {
      queue: Default::default(),
    }
  }

  pub(crate) fn push_msg(&mut self, msg_id: MsgId, msg: Msg) {
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
  msg_id: MsgId,
  state: MessageState,
  tx: Option<oneshot::Sender<MsgId>>,
}

impl<Msg> PendingMessage<Msg>
where
  Msg: Clone + Display,
{
  pub fn new(msg: Msg, msg_id: MsgId) -> Self {
    Self {
      msg,
      msg_id,
      state: MessageState::Pending,
      tx: None,
    }
  }

  pub fn get_msg(&self) -> &Msg {
    &self.msg
  }

  pub fn state(&self) -> &MessageState {
    &self.state
  }

  pub fn set_state(&mut self, new_state: MessageState) {
    self.state = new_state;
    if self.state.is_done() && self.tx.is_some() {
      self.tx.take().map(|tx| tx.send(self.msg_id));
    }
  }

  pub fn set_ret(&mut self, tx: oneshot::Sender<MsgId>) {
    self.tx = Some(tx);
  }

  pub fn msg_id(&self) -> MsgId {
    self.msg_id
  }
}

impl<Msg> PendingMessage<Msg>
where
  Msg: CollabSinkMessage,
{
  pub fn is_mergeable(&self) -> bool {
    self.msg.mergeable()
  }

  pub fn is_init(&self) -> bool {
    self.msg.is_init_msg()
  }

  pub fn merge(&mut self, other: &Self) -> bool {
    self.msg.merge(other.get_msg())
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
pub(crate) enum MessageState {
  Pending,
  Processing,
  Done,
  Timeout,
}

impl MessageState {
  pub fn is_done(&self) -> bool {
    matches!(self, MessageState::Done)
  }
  pub fn is_processing(&self) -> bool {
    matches!(self, MessageState::Processing)
  }
}
