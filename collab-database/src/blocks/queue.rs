use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::fmt::Debug;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::sync::{Arc, Weak};

use async_trait::async_trait;
use tokio::sync::watch;

pub trait RequestPayload: Clone + Ord {}

#[derive(Debug, Eq, PartialEq, Clone)]
pub enum TaskState {
  Pending,
  Processing,
  Done,
}

#[derive(Debug)]
pub struct PendingTask<Payload> {
  pub payload: Payload,
  pub state: TaskState,
}

impl<Payload> PendingTask<Payload> {
  pub fn new(payload: Payload) -> Self {
    Self {
      payload,
      state: TaskState::Pending,
    }
  }

  #[allow(dead_code)]
  pub fn state(&self) -> &TaskState {
    &self.state
  }

  pub fn set_state(&mut self, new_state: TaskState)
  where
    Payload: Debug,
  {
    if self.state != new_state {
      self.state = new_state;
    }
  }

  pub fn is_processing(&self) -> bool {
    self.state == TaskState::Processing
  }

  pub fn is_done(&self) -> bool {
    self.state == TaskState::Done
  }
}

impl<Payload> Clone for PendingTask<Payload>
where
  Payload: Clone + Debug,
{
  fn clone(&self) -> Self {
    Self {
      payload: self.payload.clone(),
      state: self.state.clone(),
    }
  }
}

pub(crate) struct TaskQueue<Payload>(BinaryHeap<PendingTask<Payload>>);

impl<Payload> TaskQueue<Payload>
where
  Payload: Ord,
{
  pub(crate) fn new() -> Self {
    Self(BinaryHeap::new())
  }
}

impl<Payload> Deref for TaskQueue<Payload>
where
  Payload: Ord,
{
  type Target = BinaryHeap<PendingTask<Payload>>;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl<Payload> DerefMut for TaskQueue<Payload>
where
  Payload: Ord,
{
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.0
  }
}

impl<Payload> Eq for PendingTask<Payload> where Payload: Eq {}

impl<Payload> PartialEq for PendingTask<Payload>
where
  Payload: PartialEq,
{
  fn eq(&self, other: &Self) -> bool {
    self.payload == other.payload
  }
}

impl<Payload> PartialOrd for PendingTask<Payload>
where
  Payload: PartialOrd + Ord,
{
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
    Some(self.cmp(other))
  }
}

impl<Payload> Ord for PendingTask<Payload>
where
  Payload: Ord,
{
  fn cmp(&self, other: &Self) -> Ordering {
    self.payload.cmp(&other.payload)
  }
}

#[async_trait]
pub trait TaskHandler<Payload>: Send + Sync + 'static {
  async fn prepare_task(&self) -> Option<PendingTask<Payload>>;
  async fn handle_task(&self, task: PendingTask<Payload>) -> Option<()>;
  fn notify(&self);
}

#[async_trait]
impl<T, Payload> TaskHandler<Payload> for Arc<T>
where
  T: TaskHandler<Payload>,
  Payload: 'static + Send + Sync,
{
  async fn prepare_task(&self) -> Option<PendingTask<Payload>> {
    (**self).prepare_task().await
  }

  async fn handle_task(&self, task: PendingTask<Payload>) -> Option<()> {
    (**self).handle_task(task).await
  }

  fn notify(&self) {
    (**self).notify()
  }
}

pub struct TaskQueueRunner<Payload>(PhantomData<Payload>);
impl<Payload> TaskQueueRunner<Payload>
where
  Payload: 'static + Send + Sync,
{
  pub async fn run(mut notifier: watch::Receiver<bool>, handler: Weak<dyn TaskHandler<Payload>>) {
    if let Some(handler) = handler.upgrade() {
      handler.notify();
    }
    loop {
      // stops the runner if the notifier was closed.
      if notifier.changed().await.is_err() {
        break;
      }

      // stops the runner if the value of notifier is `true`
      if *notifier.borrow() {
        break;
      }

      if let Some(handler) = handler.upgrade() {
        if let Some(request) = handler.prepare_task().await {
          if request.is_done() {
            handler.notify();
            continue;
          }

          if request.is_processing() {
            continue;
          }

          let _ = handler.handle_task(request).await;
          handler.notify();
        }
      } else {
        break;
      }
    }
  }
}
