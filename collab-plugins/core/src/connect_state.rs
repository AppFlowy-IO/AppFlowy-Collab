use parking_lot::Mutex;
use tokio::sync::broadcast;

#[derive(Clone, Eq, PartialEq)]
pub enum CollabConnectState {
  Connected,
  Disconnected,
}

pub struct CollabConnectReachability {
  state: Mutex<CollabConnectState>,
  state_sender: broadcast::Sender<CollabConnectState>,
}

impl Default for CollabConnectReachability {
  fn default() -> Self {
    let (state_sender, _) = broadcast::channel(1000);
    let state = Mutex::new(CollabConnectState::Connected);
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

  pub fn set_state(&self, new_state: CollabConnectState) {
    let mut lock_guard = self.state.lock();
    if *lock_guard != new_state {
      *lock_guard = new_state.clone();
      let _ = self.state_sender.send(new_state);
    }
  }

  pub fn subscribe(&self) -> broadcast::Receiver<CollabConnectState> {
    self.state_sender.subscribe()
  }
}
