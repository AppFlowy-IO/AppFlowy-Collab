use std::sync::Arc;

use parking_lot::RwLock;
use tokio::sync::watch;

#[derive(Clone, Debug)]
pub enum InitState {
  /// The [Collab] is not initialized yet. Call [Collab::initialize] to initialize
  Uninitialized,
  /// After calling [Collab::initialize] the [Collab] is in the [State::Loading] state.
  Loading,
  /// The [Collab] is initialized and ready to use.
  Initialized,
}

impl InitState {
  pub fn is_uninitialized(&self) -> bool {
    matches!(self, InitState::Uninitialized)
  }
}

/// The [SyncState] describes the steps to change the state of the [Collab] object.
/// [SyncState::SyncInitStart] -> [SyncState::SyncInitEnd] -> [SyncState::SyncUpdate] -> [SyncState::SyncFinished]
///
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SyncState {
  /// The state indicates that the [Collab] is in the process of first sync. Each [Collab]
  /// will start with the first sync.
  SyncInitStart,
  /// Init sync is finished
  SyncInitEnd,
  /// The [Collab] is in the process of syncing the data to remote
  SyncUpdate,
  /// Indicates that the [Collab] is finished syncing the data to remote. All local updates
  /// are sent to the remote.
  SyncFinished,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SnapshotState {
  WaitingForSnapshot,
  DidCreateSnapshot { snapshot_id: i64 },
}

impl SnapshotState {
  pub fn snapshot_id(&self) -> Option<i64> {
    match self {
      SnapshotState::WaitingForSnapshot => None,
      SnapshotState::DidCreateSnapshot { snapshot_id } => Some(*snapshot_id),
    }
  }
}

impl SyncState {
  pub fn is_sync_finished(&self) -> bool {
    matches!(self, SyncState::SyncFinished)
  }

  pub fn is_syncing(&self) -> bool {
    !self.is_sync_finished()
  }
}

pub struct State {
  object_id: String,
  init_state: Arc<RwLock<InitState>>,
  sync_state: Arc<RwLock<SyncState>>,
  snapshot_state: Arc<RwLock<SnapshotState>>,
  pub(crate) sync_state_notifier: Arc<watch::Sender<SyncState>>,
  pub(crate) snapshot_state_notifier: Arc<watch::Sender<SnapshotState>>,
}

impl State {
  pub fn new(object_id: &str) -> Self {
    let (sync_state_notifier, _) = watch::channel(SyncState::SyncInitStart);
    let (snapshot_state_notifier, _) = watch::channel(SnapshotState::WaitingForSnapshot);
    Self {
      object_id: object_id.to_string(),
      init_state: Arc::new(RwLock::new(InitState::Uninitialized)),
      sync_state: Arc::new(RwLock::new(SyncState::SyncInitStart)),
      snapshot_state: Arc::new(RwLock::new(SnapshotState::WaitingForSnapshot)),
      sync_state_notifier: Arc::new(sync_state_notifier),
      snapshot_state_notifier: Arc::new(snapshot_state_notifier),
    }
  }

  pub fn get(&self) -> InitState {
    self.init_state.read().clone()
  }

  pub fn is_uninitialized(&self) -> bool {
    self.get().is_uninitialized()
  }

  pub fn set_init_state(&self, state: InitState) {
    *self.init_state.write() = state;
  }

  pub fn set_sync_state(&self, new_state: SyncState) {
    let old_state = self.sync_state.read().clone();
    if old_state != new_state {
      if new_state.is_sync_finished() {
        tracing::debug!("{} sync finish 🌐", self.object_id,);
      } else {
        tracing::trace!(
          "{} sync state {:?} => {:?}",
          self.object_id,
          old_state,
          new_state
        );
      }

      *self.sync_state.write() = new_state.clone();
      let _ = self.sync_state_notifier.send(new_state);
    }
  }

  pub fn set_snapshot_state(&self, new_state: SnapshotState) {
    let old_state = self.snapshot_state.read().clone();
    if old_state != new_state {
      *self.snapshot_state.write() = new_state.clone();
      let _ = self.snapshot_state_notifier.send(new_state);
    }
  }
}
