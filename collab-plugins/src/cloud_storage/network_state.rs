use parking_lot::Mutex;
use tokio::sync::broadcast;

#[derive(Clone, Eq, PartialEq)]
pub enum CollabNetworkState {
  Connected,
  Disconnected,
}

pub struct CollabNetworkReachability {
  state: Mutex<CollabNetworkState>,
  state_sender: broadcast::Sender<CollabNetworkState>,
}

impl Default for CollabNetworkReachability {
  fn default() -> Self {
    let (state_sender, _) = broadcast::channel(1000);
    let state = Mutex::new(CollabNetworkState::Connected);
    Self {
      state,
      state_sender,
    }
  }
}

impl CollabNetworkReachability {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn set_state(&self, new_state: CollabNetworkState) {
    let mut lock_guard = self.state.lock();
    if *lock_guard != new_state {
      *lock_guard = new_state.clone();
      let _ = self.state_sender.send(new_state);
    }
  }

  pub fn subscribe(&self) -> broadcast::Receiver<CollabNetworkState> {
    self.state_sender.subscribe()
  }
}
