use std::sync::atomic::{AtomicU8, Ordering};
use tokio::sync::broadcast;

#[repr(u8)]
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum CollabConnectState {
  Connected = CollabConnectState::CONNECTED,
  Disconnected = CollabConnectState::DISCONNECTED,
}

impl CollabConnectState {
  const CONNECTED: u8 = 0;
  const DISCONNECTED: u8 = 1;
}

impl TryFrom<u8> for CollabConnectState {
  type Error = u8;

  fn try_from(value: u8) -> Result<Self, Self::Error> {
    match value {
      Self::CONNECTED => Ok(Self::Connected),
      Self::DISCONNECTED => Ok(Self::Disconnected),
      unknown => Err(unknown),
    }
  }
}

pub struct CollabConnectReachability {
  state: AtomicU8,
  state_sender: broadcast::Sender<CollabConnectState>,
}

impl Default for CollabConnectReachability {
  fn default() -> Self {
    let (state_sender, _) = broadcast::channel(1000);
    let state = AtomicU8::new(CollabConnectState::Connected as u8);
    Self {
      state,
      state_sender,
    }
  }
}

impl CollabConnectReachability {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn state(&self) -> CollabConnectState {
    CollabConnectState::try_from(self.state.load(Ordering::Acquire)).unwrap()
  }

  pub fn set_state(&self, new_state: CollabConnectState) {
    let old = self.state.swap(new_state as u8, Ordering::AcqRel);
    if old != new_state as u8 {
      let _ = self.state_sender.send(new_state);
    }
  }

  pub fn subscribe(&self) -> broadcast::Receiver<CollabConnectState> {
    self.state_sender.subscribe()
  }
}
