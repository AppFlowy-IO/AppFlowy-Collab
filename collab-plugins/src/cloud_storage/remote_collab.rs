use std::cmp::Ordering;
use std::fmt::{Display, Formatter};
use std::sync::Arc;
use std::time::SystemTime;

use async_trait::async_trait;
use collab::core::collab::MutexCollab;
use collab::core::origin::CollabOrigin;
use collab_sync::client::sink::{
  CollabSink, CollabSinkMessage, CollabSinkRunner, MsgId, MsgIdCounter, SinkConfig,
};
use collab_sync::client::TokioUnboundedSink;
use parking_lot::Mutex;
use rand::Rng;
use tokio::spawn;
use tokio::sync::mpsc::unbounded_channel;
use tokio::sync::watch;
use yrs::updates::decoder::Decode;
use yrs::{merge_updates_v1, ReadTxn, Transact, Update};

/// The [RemoteCollabStorage] is used to store the updates of the remote collab. The [RemoteCollab]
/// is the remote collab that maps to the local collab.
/// Any storage that implements this trait can be used as the remote collab storage.
#[async_trait]
pub trait RemoteCollabStorage: Send + Sync + 'static {
  /// Get all the updates of the remote collab.
  async fn get_all_updates(&self, object_id: &str) -> Result<Vec<Vec<u8>>, anyhow::Error>;
  /// Send the update to the remote storage.
  async fn send_update(&self, id: MsgId, update: Vec<u8>) -> Result<(), anyhow::Error>;
}

/// The [RemoteCollab] is used to sync the local collab to the remote.
pub struct RemoteCollab {
  object: CollabObject,
  collab: Arc<MutexCollab>,
  storage: Arc<dyn RemoteCollabStorage>,
  /// The [CollabSink] is used to send the updates to the remote.
  sink: Arc<CollabSink<TokioUnboundedSink<Message>, Message>>,
}

impl RemoteCollab {
  /// Create a new remote collab.
  /// `timeout` is the time to wait for the server to ack the message.
  /// If the server does not ack the message in time, the message will be sent again.
  pub fn new<S>(object: CollabObject, storage: S, config: SinkConfig) -> Self
  where
    S: RemoteCollabStorage + Send + Sync + 'static,
  {
    let storage: Arc<dyn RemoteCollabStorage> = Arc::new(storage);
    let collab = Arc::new(MutexCollab::new(CollabOrigin::Server, &object.id, vec![]));
    let (sink, mut stream) = unbounded_channel::<Message>();
    let weak_storage = Arc::downgrade(&storage);
    let (notifier, notifier_rx) = watch::channel(false);
    let sink = Arc::new(CollabSink::new(
      TokioUnboundedSink(sink),
      notifier,
      RngMsgIdCounter::new(),
      config,
    ));

    let weak_sink = Arc::downgrade(&sink);
    spawn(async move {
      while let Some(message) = stream.recv().await {
        if let Some(storage) = weak_storage.upgrade() {
          if let Ok((object, msg_id, payload)) = message.split() {
            match storage.send_update(msg_id, payload).await {
              Ok(_) => {
                tracing::debug!("ack update {}: {}", object, msg_id);
                if let Some(sink) = weak_sink.upgrade() {
                  sink.ack_msg(msg_id).await;
                }
              },
              Err(e) => {
                tracing::error!("send {} update failed: {:?}", msg_id, e);
              },
            }
          } else {
            tracing::error!("Failed to get the payload from message");
          }
        }
      }
    });

    spawn(CollabSinkRunner::run(Arc::downgrade(&sink), notifier_rx));
    Self {
      object,
      collab,
      storage,
      sink,
    }
  }

  pub async fn sync(&self, local_collab: Arc<MutexCollab>) {
    tracing::trace!("{}: start sync with remote", self.object);
    let updates = match self.storage.get_all_updates(&self.object.id).await {
      Ok(updates) => updates,
      Err(e) => {
        tracing::error!("🔴Failed to get updates: {:?}", e);
        vec![]
      },
    };

    tracing::trace!(
      "{}: try apply remote updates with len:{}",
      self.object,
      updates.len(),
    );

    if !updates.is_empty() {
      // Apply remote updates to remote collab before encode the state as update
      // for local collab.
      self.collab.lock().with_transact_mut(|txn| {
        for update in updates {
          if let Ok(update) = Update::decode_v1(&update) {
            txn.apply_update(update);
          } else {
            tracing::error!("Failed to decode update");
          }
        }
      });

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
          // TODO: Will define a sync protocol for cloud storage.
          tracing::trace!(
            "{}: apply remote update with len:{}",
            self.object,
            encode_update.len()
          );
          let local_collab_guard = local_collab.lock();
          let mut txn = local_collab_guard.get_doc().transact_mut();
          txn.apply_update(update);
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
        "{}: try sync local update with len:{}",
        self.object,
        encode_update.len()
      );

      self.collab.lock().with_transact_mut(|txn| {
        txn.apply_update(decode_update);
      });

      self.sink.queue_msg(|msg_id| Message {
        object: self.object.clone(),
        payloads: vec![encode_update],
        meta: MessageMeta::Init { msg_id },
      });
    }
  }

  pub fn push_update(&self, update: &[u8]) {
    if let Ok(decode_update) = Update::decode_v1(update) {
      self.collab.lock().with_transact_mut(|txn| {
        txn.apply_update(decode_update);
      });

      self.sink.queue_or_merge_msg(
        |prev| {
          prev.merge_payload(update.to_vec());
          Ok(())
        },
        |msg_id| Message {
          object: self.object.clone(),
          payloads: vec![update.to_vec()],
          meta: MessageMeta::Update { msg_id },
        },
      );
    }
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
  fn merge_payload(&mut self, payload: Vec<u8>) {
    self.payloads.push(payload);
  }

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
      MessageMeta::Update { .. } => self.payload_len() < 1024,
    }
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
      ) => msg_id_a.cmp(msg_id_b),
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
  pub(crate) id: String,
  name: String,
}

impl CollabObject {
  pub fn new(object_id: String) -> Self {
    Self {
      id: object_id,
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
