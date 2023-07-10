use std::cmp::Ordering;
use std::fmt::{Display, Formatter};
use std::sync::Arc;
use std::time::SystemTime;

use anyhow::Error;
use async_trait::async_trait;
use collab::core::collab::MutexCollab;
use collab::core::collab_state::SyncState;
use collab::core::origin::CollabOrigin;
use collab_persistence::TransactionMutExt;
pub use collab_sync::client::sink::MsgId;
use collab_sync::client::sink::{
  CollabSink, CollabSinkMessage, CollabSinkRunner, MsgIdCounter, SinkConfig, SinkState,
};
use collab_sync::client::TokioUnboundedSink;
use parking_lot::Mutex;
use rand::Rng;
use tokio::spawn;
use tokio::sync::mpsc::unbounded_channel;
use tokio::sync::watch;
use tokio_stream::wrappers::WatchStream;
use tokio_stream::StreamExt;
use yrs::updates::decoder::Decode;
use yrs::{merge_updates_v1, ReadTxn, Transact, Update};

/// The [RemoteCollab] is used to sync the local collab to the remote.
pub struct RemoteCollab {
  object: CollabObject,
  collab: Arc<MutexCollab>,
  storage: Arc<dyn RemoteCollabStorage>,
  /// The [CollabSink] is used to queue the [Message] and continuously try to send them
  /// to the remote via the [RemoteCollabStorage].
  sink: Arc<CollabSink<TokioUnboundedSink<Message>, Message>>,
  /// It continuously receive the updates from the remote.
  sync_state: Arc<watch::Sender<SyncState>>,
}

impl Drop for RemoteCollab {
  fn drop(&mut self) {
    tracing::trace!("{} remote collab dropped", self.object);
  }
}

impl RemoteCollab {
  /// Create a new remote collab.
  /// `timeout` is the time to wait for the server to ack the message.
  /// If the server does not ack the message in time, the message will be sent again.
  pub fn new(
    object: CollabObject,
    storage: Arc<dyn RemoteCollabStorage>,
    config: SinkConfig,
  ) -> Self {
    let sync_state = Arc::new(watch::channel(SyncState::SyncInitStart).0);
    let collab = Arc::new(MutexCollab::new(CollabOrigin::Server, &object.id, vec![]));
    let (sink, mut stream) = unbounded_channel::<Message>();
    let weak_storage = Arc::downgrade(&storage);
    let (notifier, notifier_rx) = watch::channel(false);
    let (sync_state_tx, sink_state_rx) = watch::channel(SinkState::Init);
    let sink = Arc::new(CollabSink::new(
      TokioUnboundedSink(sink),
      notifier,
      sync_state_tx,
      RngMsgIdCounter::new(),
      config,
    ));

    let weak_sink = Arc::downgrade(&sink);
    let weak_sync_state = Arc::downgrade(&sync_state);
    let mut sink_state_stream = WatchStream::new(sink_state_rx);
    // Subscribe the sink state stream and update the sync state in the background.
    spawn(async move {
      while let Some(collab_state) = sink_state_stream.next().await {
        if let Some(sync_state) = weak_sync_state.upgrade() {
          match collab_state {
            SinkState::Syncing => {
              let _ = sync_state.send(SyncState::SyncUpdate);
            },
            SinkState::Finished => {
              let _ = sync_state.send(SyncState::SyncFinished);
            },
            SinkState::Init => {
              let _ = sync_state.send(SyncState::SyncInitStart);
            },
          }
        }
      }
    });

    // Spawn a task to receive updates from the [CollabSink] and send updates to
    // the remote storage.
    spawn(async move {
      while let Some(message) = stream.recv().await {
        if let Some(storage) = weak_storage.upgrade() {
          let is_init_msg = message.is_init_msg();
          match message.split() {
            Ok((object, msg_id, payload)) => {
              // If the message is init message, it will flush all the updates to the remote.
              if is_init_msg {
                tracing::trace!("send init sync {}:{}", object, msg_id);
                match storage.send_init_sync(&object, msg_id, payload).await {
                  Ok(_) => {
                    if let Some(sink) = weak_sink.upgrade() {
                      sink.ack_msg(msg_id).await;
                    }
                  },
                  Err(e) => {
                    tracing::error!("send {}:{} init sync failed: {:?}", object.id, msg_id, e)
                  },
                }
              } else {
                tracing::trace!("send update {}:{}", object, msg_id);
                match storage.send_update(&object, msg_id, payload).await {
                  Ok(_) => {
                    tracing::debug!("ack update {}:{}", object, msg_id);
                    if let Some(sink) = weak_sink.upgrade() {
                      sink.ack_msg(msg_id).await;
                    }
                  },
                  Err(e) => tracing::error!("send {}:{} update failed: {:?}", object.id, msg_id, e),
                }
              }
            },
            Err(e) => tracing::error!("🔴Failed to split message: {:?}", e),
          }
        }
      }
    });

    // Spawn a task that boost the [CollabSink]
    spawn(CollabSinkRunner::run(Arc::downgrade(&sink), notifier_rx));
    Self {
      object,
      collab,
      storage,
      sink,
      sync_state,
    }
  }

  pub fn subscribe_sync_state(&self) -> watch::Receiver<SyncState> {
    self.sync_state.subscribe()
  }

  /// Return the update of the remote collab.
  /// If the remote collab contains any updates, it will return None.
  /// Otherwise, it will merge the updates into one and return the merged update.
  pub async fn sync(&self, local_collab: Arc<MutexCollab>) -> Option<Vec<u8>> {
    let mut remote_update = None;
    // It would be better if creating a edge function that calculate the diff between the local and remote.
    // The local only need to send its state vector to the remote. In this way, the local does not need to
    // get all the updates from remote.
    // TODO(nathan): create a edge function to calculate the diff between the local and remote.
    let remote_updates = match self.storage.get_all_updates(&self.object.id).await {
      Ok(updates) => updates,
      Err(e) => {
        tracing::error!("🔴Failed to get updates: {:?}", e);
        vec![]
      },
    };

    if !remote_updates.is_empty() {
      let updates = remote_updates
        .iter()
        .map(|update| update.as_ref())
        .collect::<Vec<&[u8]>>();

      if let Ok(update) = merge_updates_v1(&updates) {
        tracing::trace!("{}: sync remote updates:{}", self.object, update.len());
        // Restore the remote collab state from updates
        {
          let remote_collab = self.collab.lock();
          let mut txn = remote_collab.transact_mut();
          if let Ok(update) = Update::decode_v1(&update) {
            if let Err(e) = txn.try_apply_update(update) {
              tracing::error!("apply update failed: {:?}", e);
            }
          } else {
            tracing::error!("🔴decode update failed");
          }

          remote_update = Some(update);
        }
      }

      let _ = self.sync_state.send(SyncState::SyncInitStart);
      // Encode the remote collab state as update for local collab.
      let local_sv = local_collab.lock().transact().state_vector();
      let encode_update = self
        .collab
        .lock()
        .transact()
        .encode_state_as_update_v1(&local_sv);
      if let Ok(update) = Update::decode_v1(&encode_update) {
        {
          // Don't use the with_transact_mut here, because it carries the origin information. So
          // the update will consider as a local update. But here is apply the remote update.
          // TODO: nathan define a sync protocol for cloud storage.
          tracing::trace!(
            "{}: apply remote update with diff len:{}",
            self.object,
            encode_update.len()
          );
          let local_collab_guard = local_collab.lock();
          let mut txn = local_collab_guard.get_doc().transact_mut();
          txn.apply_update(update);
          drop(txn);

          if let Err(e) = self.sync_state.send(SyncState::SyncInitEnd) {
            tracing::error!("🔴Failed to send sync state: {:?}", e);
          }
        }
      }
    }

    // Encode the local collab state as update for remote collab.
    let remote_state_vector = self.collab.lock().transact().state_vector();
    let encode_update = local_collab
      .lock()
      .transact()
      .encode_state_as_update_v1(&remote_state_vector);

    if let Ok(decode_update) = Update::decode_v1(&encode_update) {
      tracing::trace!(
        "{}: sync updates to remote:{}",
        self.object,
        encode_update.len()
      );

      // Apply the update to the remote collab and send the update to the remote.
      self.collab.lock().with_transact_mut(|txn| {
        txn.apply_update(decode_update);
      });

      self.sink.queue_msg(|msg_id| Message {
        object: self.object.clone(),
        payloads: vec![encode_update],
        meta: MessageMeta::Init { msg_id },
      });
    }
    remote_update
  }

  pub fn push_update(&self, update: &[u8]) {
    if let Ok(decode_update) = Update::decode_v1(update) {
      self.collab.lock().with_transact_mut(|txn| {
        txn.apply_update(decode_update);
      });

      self.sink.queue_msg(|msg_id| Message {
        object: self.object.clone(),
        payloads: vec![update.to_vec()],
        meta: MessageMeta::Update { msg_id },
      });
    }
  }

  pub fn clear(&self) {
    self.sink.remove_all_pending_msgs();
  }
}

#[derive(Debug, Clone)]
pub struct RemoteCollabState {
  /// The current edit count of the remote collab.
  pub current_edit_count: i64,
  /// The last edit count of the remote collab when the snapshot is created.
  pub last_snapshot_edit_count: i64,
  /// The last snapshot of the remote collab.
  pub last_snapshot_created_at: i64,
}

pub fn should_create_snapshot(state: &RemoteCollabState) -> bool {
  state.current_edit_count > state.last_snapshot_edit_count + 50
}

pub struct RemoteCollabSnapshot {
  pub snapshot_id: i64,
  pub oid: String,
  pub data: Vec<u8>,
  pub created_at: i64,
}

/// The [RemoteCollabStorage] is used to store the updates of the remote collab. The [RemoteCollab]
/// is the remote collab that maps to the local collab.
/// Any storage that implements this trait can be used as the remote collab storage.
#[async_trait]
pub trait RemoteCollabStorage: Send + Sync + 'static {
  /// Get all the updates of the remote collab.
  async fn get_all_updates(&self, object_id: &str) -> Result<Vec<Vec<u8>>, anyhow::Error>;

  /// Get the latest snapshot of the remote collab.
  async fn get_latest_snapshot(
    &self,
    object_id: &str,
  ) -> Result<Option<RemoteCollabSnapshot>, anyhow::Error>;

  /// Return the remote state of the collab. It contains the current edit count, the last snapshot
  /// edit count and the last snapshot created time.
  async fn get_collab_state(
    &self,
    object_id: &str,
  ) -> Result<Option<RemoteCollabState>, anyhow::Error>;

  /// Create a snapshot of the remote collab. The update contains the full state of the [Collab]
  async fn create_snapshot(
    &self,
    object: &CollabObject,
    snapshot: Vec<u8>,
  ) -> Result<i64, anyhow::Error>;

  /// Send the update to the remote storage.
  async fn send_update(
    &self,
    object: &CollabObject,
    id: MsgId,
    update: Vec<u8>,
  ) -> Result<(), anyhow::Error>;

  /// The init sync is used to send the initial state of the remote collab to the remote storage.
  /// The init_update contains all the missing updates of the remote collab compared to the local.
  async fn send_init_sync(
    &self,
    object: &CollabObject,
    id: MsgId,
    init_update: Vec<u8>,
  ) -> Result<(), anyhow::Error>;

  /// Subscribe the remote updates.
  async fn subscribe_remote_updates(&self, object: &CollabObject) -> Option<RemoteUpdateReceiver>;
}

pub type RemoteUpdateReceiver = tokio::sync::mpsc::Receiver<Vec<u8>>;

#[async_trait]
impl<T> RemoteCollabStorage for Arc<T>
where
  T: RemoteCollabStorage,
{
  async fn get_all_updates(&self, object_id: &str) -> Result<Vec<Vec<u8>>, Error> {
    (**self).get_all_updates(object_id).await
  }

  async fn get_latest_snapshot(
    &self,
    object_id: &str,
  ) -> Result<Option<RemoteCollabSnapshot>, Error> {
    (**self).get_latest_snapshot(object_id).await
  }

  async fn get_collab_state(&self, object_id: &str) -> Result<Option<RemoteCollabState>, Error> {
    (**self).get_collab_state(object_id).await
  }

  async fn create_snapshot(&self, object: &CollabObject, update: Vec<u8>) -> Result<i64, Error> {
    (**self).create_snapshot(object, update).await
  }

  async fn send_update(
    &self,
    object: &CollabObject,
    id: MsgId,
    update: Vec<u8>,
  ) -> Result<(), Error> {
    (**self).send_update(object, id, update).await
  }

  async fn send_init_sync(
    &self,
    object: &CollabObject,
    id: MsgId,
    init_update: Vec<u8>,
  ) -> Result<(), Error> {
    (**self).send_init_sync(object, id, init_update).await
  }

  async fn subscribe_remote_updates(&self, object: &CollabObject) -> Option<RemoteUpdateReceiver> {
    (**self).subscribe_remote_updates(object).await
  }
}

#[derive(Clone, Debug)]
pub enum MessageMeta {
  Init { msg_id: MsgId },
  Update { msg_id: MsgId },
}

impl MessageMeta {
  pub fn msg_id(&self) -> &MsgId {
    match self {
      Self::Init { msg_id, .. } => msg_id,
      Self::Update { msg_id, .. } => msg_id,
    }
  }

  pub fn is_init(&self) -> bool {
    matches!(self, Self::Init { .. })
  }
}

/// A message that is sent to the remote.
#[derive(Clone, Debug)]
struct Message {
  object: CollabObject,
  meta: MessageMeta,
  payloads: Vec<Vec<u8>>,
}

impl Message {
  fn payload_len(&self) -> usize {
    self.payloads.iter().map(|p| p.len()).sum()
  }

  fn split(self) -> Result<(CollabObject, MsgId, Vec<u8>), anyhow::Error> {
    let updates = self
      .payloads
      .iter()
      .map(|update| update.as_ref())
      .collect::<Vec<&[u8]>>();
    let update = merge_updates_v1(&updates)?;
    let msg_id = *self.meta.msg_id();
    Ok((self.object, msg_id, update))
  }
}

impl CollabSinkMessage for Message {
  fn length(&self) -> usize {
    self.payload_len()
  }

  fn mergeable(&self) -> bool {
    match self.meta {
      MessageMeta::Init { .. } => false,
      // Special characters, emojis, and characters from many other languages can take 2, 3, or
      // even 4 bytes in UTF-8. So assuming that these are standard English characters and encoded
      // using UTF-8, each character will take 1 byte. 4096 can hold 4096 characters.
      // The default max message size is 4kb.
      MessageMeta::Update { .. } => self.payload_len() < (1024 * 4),
    }
  }

  fn merge(&mut self, other: Self) {
    self.payloads.extend(other.payloads);
  }

  fn is_init_msg(&self) -> bool {
    matches!(self.meta, MessageMeta::Init { .. })
  }

  fn deferrable(&self) -> bool {
    // If the message is not init message, it can be pending.
    !self.meta.is_init()
  }
}

impl Eq for Message {}

impl PartialEq for Message {
  fn eq(&self, other: &Self) -> bool {
    self.meta.msg_id() == other.meta.msg_id()
  }
}

impl PartialOrd for Message {
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
    Some(self.cmp(other))
  }
}

impl Ord for Message {
  fn cmp(&self, other: &Self) -> Ordering {
    // Init message has higher priority than update message.
    match (&self.meta, &other.meta) {
      (MessageMeta::Init { msg_id: msg_id_a }, MessageMeta::Init { msg_id: msg_id_b }) => {
        msg_id_a.cmp(msg_id_b)
      },
      (MessageMeta::Init { .. }, MessageMeta::Update { .. }) => Ordering::Greater,
      (MessageMeta::Update { .. }, MessageMeta::Init { .. }) => Ordering::Less,
      (
        MessageMeta::Update {
          msg_id: msg_id_a, ..
        },
        MessageMeta::Update {
          msg_id: msg_id_b, ..
        },
      ) => msg_id_a.cmp(msg_id_b).reverse(),
    }
  }
}

impl Display for Message {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    f.write_fmt(format_args!(
      "{} update: [msg_id:{}|payload_len:{}]",
      self.object,
      self.meta.msg_id(),
      self.payload_len(),
    ))
  }
}

#[derive(Debug, thiserror::Error)]
enum CollabError {
  #[error("Internal error")]
  Internal(#[from] anyhow::Error),
}

const RANDOM_MASK: u64 = (1 << 12) - 1;

struct RngMsgIdCounter(Mutex<MsgId>);

impl RngMsgIdCounter {
  pub fn new() -> Self {
    let timestamp = SystemTime::now()
      .duration_since(SystemTime::UNIX_EPOCH)
      .expect("Clock moved backwards!")
      .as_millis() as u64;

    let random: u64 = (rand::thread_rng().gen::<u16>() as u64) & RANDOM_MASK;
    let value = timestamp << 16 | random;
    Self(Mutex::new(value))
  }
}

impl MsgIdCounter for RngMsgIdCounter {
  fn next(&self) -> MsgId {
    let next = *self.0.lock() + 1;
    *self.0.lock() = next;
    next
  }
}

#[derive(Clone, Debug)]
pub struct CollabObject {
  pub id: String,
  pub uid: i64,
  pub name: String,
}

impl CollabObject {
  pub fn new(uid: i64, object_id: String) -> Self {
    Self {
      id: object_id,
      uid,
      name: "".to_string(),
    }
  }

  pub fn with_name(mut self, name: &str) -> Self {
    self.name = name.to_string();
    self
  }
}

impl Display for CollabObject {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    f.write_fmt(format_args!("{}:{}]", self.name, self.id,))
  }
}
