use arc_swap::ArcSwap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use tokio::sync::watch;

#[repr(u32)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum InitState {
  /// The [Collab] is not initialized yet. Call [Collab::initialize] to initialize
  Uninitialized = InitState::UNINITIALIZED,
  /// After calling [Collab::initialize] the [Collab] is in the [State::Loading] state.
  Loading = InitState::LOADING,
  /// The [Collab] is initialized and ready to use.
  Initialized = InitState::INITIALIZED,
}

impl InitState {
  const UNINITIALIZED: u32 = 0;
  const LOADING: u32 = 1;
  const INITIALIZED: u32 = 2;

  #[inline]
  pub fn is_uninitialized(&self) -> bool {
    *self == InitState::Uninitialized
  }
}

impl TryFrom<u32> for InitState {
  type Error = u32;

  fn try_from(value: u32) -> Result<Self, Self::Error> {
    match value {
      Self::UNINITIALIZED => Ok(Self::Uninitialized),
      Self::LOADING => Ok(Self::Loading),
      Self::INITIALIZED => Ok(Self::Initialized),
      unknown => Err(unknown),
    }
  }
}

/// The [SyncState] describes the steps to change the state of the [Collab] object.
/// [SyncState::InitSyncBegin] -> [SyncState::InitSyncEnd] -> [SyncState::Syncing] -> [SyncState::SyncFinished]
#[repr(u32)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum SyncState {
  /// The state indicates that the [Collab] is in the process of first sync. Each [Collab]
  /// will start with the first sync.
  InitSyncBegin = SyncState::INIT_SYNC_BEGIN,
  /// Init sync is finished
  InitSyncEnd = SyncState::INIT_SYNC_END,
  /// The [Collab] is in the process of syncing the data to remote
  Syncing = SyncState::SYNCING,
  /// Indicates that the [Collab] is finished syncing the data to remote. All local updates
  /// are sent to the remote.
  SyncFinished = SyncState::SYNC_FINISHED,
}

impl SyncState {
  const INIT_SYNC_BEGIN: u32 = 0;
  const INIT_SYNC_END: u32 = 1;
  const SYNCING: u32 = 2;
  const SYNC_FINISHED: u32 = 3;

  #[inline]
  pub fn is_sync_finished(&self) -> bool {
    *self == SyncState::SyncFinished
  }

  #[inline]
  pub fn is_syncing(&self) -> bool {
    !self.is_sync_finished()
  }
}

impl TryFrom<u32> for SyncState {
  type Error = u32;

  fn try_from(value: u32) -> Result<Self, Self::Error> {
    match value {
      Self::INIT_SYNC_BEGIN => Ok(Self::InitSyncBegin),
      Self::INIT_SYNC_END => Ok(Self::InitSyncEnd),
      Self::SYNCING => Ok(Self::Syncing),
      Self::SYNC_FINISHED => Ok(Self::SyncFinished),
      unknown => Err(unknown),
    }
  }
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

pub struct State {
  object_id: String,
  init_state: AtomicU32,
  sync_state: AtomicU32,
  snapshot_state: ArcSwap<SnapshotState>,
  pub(crate) sync_state_notifier: Arc<watch::Sender<SyncState>>,
  pub(crate) snapshot_state_notifier: Arc<watch::Sender<SnapshotState>>,
}

impl State {
  pub fn new(object_id: &str) -> Self {
    let (sync_state_notifier, _) = watch::channel(SyncState::InitSyncBegin);
    let (snapshot_state_notifier, _) = watch::channel(SnapshotState::WaitingForSnapshot);
    Self {
      object_id: object_id.to_string(),
      init_state: AtomicU32::new(InitState::Uninitialized as u32),
      sync_state: AtomicU32::new(SyncState::InitSyncBegin as u32),
      snapshot_state: ArcSwap::new(SnapshotState::WaitingForSnapshot.into()),
      sync_state_notifier: Arc::new(sync_state_notifier),
      snapshot_state_notifier: Arc::new(snapshot_state_notifier),
    }
  }

  pub fn get(&self) -> InitState {
    InitState::try_from(self.init_state.load(Ordering::Acquire)).unwrap()
  }

  pub fn is_uninitialized(&self) -> bool {
    self.get().is_uninitialized()
  }

  pub fn sync_state(&self) -> SyncState {
    SyncState::try_from(self.sync_state.load(Ordering::Acquire)).unwrap()
  }

  pub fn is_sync_finished(&self) -> bool {
    self.sync_state().is_sync_finished()
  }

  pub fn set_init_state(&self, state: InitState) {
    self.init_state.store(state as u32, Ordering::Release);
  }

  pub fn set_sync_state(&self, new_state: SyncState) {
    let old_state =
      SyncState::try_from(self.sync_state.swap(new_state as u32, Ordering::AcqRel)).unwrap();

    if old_state != new_state {
      tracing::debug!(
        "{} sync state {:?} => {:?}",
        self.object_id,
        old_state,
        new_state
      );

      let _ = self.sync_state_notifier.send(new_state);
    }
  }

  pub fn set_snapshot_state(&self, new_state: SnapshotState) {
    let old_state = self.snapshot_state.swap(new_state.clone().into());
    if *old_state != new_state {
      let _ = self.snapshot_state_notifier.send(new_state);
    }
  }
}
