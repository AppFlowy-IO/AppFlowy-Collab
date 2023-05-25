use parking_lot::RwLock;
use std::sync::Arc;
use tokio::sync::watch;

#[derive(Clone, Debug)]
pub enum CollabState {
  /// The [Collab] is not initialized yet. Call [Collab::initialize] to initialize
  Uninitialized,
  /// After calling [Collab::initialize] the [Collab] is in the [State::Loading] state.
  Loading,
  /// The root of [Collab] was changed. This happens when root that hold by the `data` property
  /// of the [Collab] was updated by the remote.
  ///
  /// For example, when opening a document with empty data section, the [Collab] will try to load
  /// the all the data from the remote if it has the cloud storage plugin. When the remote
  /// update(The full data) is received, the root will be reset. When the root is reset, the [Collab]
  /// will emit [CollabEvent::RootChanged] event and the subscribers must reload themselves.
  ///
  RootChanged,
  /// The [Collab] is initialized and ready to use.
  Initialized,
}

impl CollabState {
  pub fn is_uninitialized(&self) -> bool {
    matches!(self, CollabState::Uninitialized)
  }

  pub fn is_root_changed(&self) -> bool {
    matches!(self, CollabState::RootChanged)
  }
}

pub struct State {
  object_id: String,
  inner: Arc<RwLock<CollabState>>,
  pub(crate) notifier: Arc<watch::Sender<CollabState>>,
}

impl State {
  pub fn new(object_id: &str) -> Self {
    let (state_notifier, _) = watch::channel(CollabState::Uninitialized);
    Self {
      object_id: object_id.to_string(),
      inner: Arc::new(RwLock::new(CollabState::Uninitialized)),
      notifier: Arc::new(state_notifier),
    }
  }

  pub fn get(&self) -> CollabState {
    self.inner.read().clone()
  }

  pub fn is_uninitialized(&self) -> bool {
    self.get().is_uninitialized()
  }

  pub fn set(&self, state: CollabState) {
    tracing::trace!(
      "[🦀Collab] {} state did change from {:?} to {:?}",
      self.object_id,
      self.inner.read(),
      state
    );
    *self.inner.write() = state.clone();
    let _ = self.notifier.send(state);
  }
}
