use std::cmp::Ordering;
use std::fmt::{Debug, Formatter};
use std::sync::{Arc, Weak};

use async_trait::async_trait;
use collab::core::collab::{DocStateSource, MutexCollab};
use collab::core::origin::CollabOrigin;
use collab_entity::CollabType;
use collab_plugins::local_storage::kv::doc::CollabKVAction;
use collab_plugins::local_storage::kv::KVTransactionDB;
use collab_plugins::local_storage::CollabPersistenceConfig;
use collab_plugins::CollabKVDB;
use tokio::sync::watch;
use tokio::task::yield_now;
use tracing::trace;

use crate::blocks::queue::{
  PendingTask, RequestPayload, TaskHandler, TaskQueue, TaskQueueRunner, TaskState,
};
use crate::rows::{DatabaseRow, RowDetail, RowId};
use crate::workspace_database::DatabaseCollabService;

/// A [BlockTaskController] is used to control how the [BlockTask]s are executed.
/// It contains a [TaskQueue] to queue the [BlockTask]s and a [TaskHandler] to handle the
/// [BlockTask]s.
///
pub struct BlockTaskController {
  request_handler: Arc<BlockTaskHandler>,
}

impl BlockTaskController {
  pub fn new(collab_db: Weak<CollabKVDB>, collab_service: Weak<dyn DatabaseCollabService>) -> Self {
    let (runner_notifier_tx, runner_notifier) = watch::channel(false);
    let task_handler = Arc::new(BlockTaskHandler::new(
      collab_service,
      collab_db,
      runner_notifier_tx,
    ));

    let handler = Arc::downgrade(&task_handler) as Weak<dyn TaskHandler<BlockTask>>;
    tokio::spawn(TaskQueueRunner::run(runner_notifier, handler));

    Self {
      request_handler: task_handler,
    }
  }

  /// Add a new task to the queue. The task with higher sequence number will be executed first.
  /// Just like Last In First Out (LIFO).
  pub fn add_task(&self, task: BlockTask) {
    self
      .request_handler
      .queue
      .lock()
      .push(PendingTask::new(task));
    self.request_handler.notify();
  }
}

pub struct BlockTaskHandler {
  collab_db: Weak<CollabKVDB>,
  collab_service: Weak<dyn DatabaseCollabService>,
  queue: parking_lot::Mutex<TaskQueue<BlockTask>>,
  runner_notifier: Arc<watch::Sender<bool>>,
}

impl BlockTaskHandler {
  pub fn new(
    collab_service: Weak<dyn DatabaseCollabService>,
    collab_db: Weak<CollabKVDB>,
    runner_notifier: watch::Sender<bool>,
  ) -> Self {
    let queue = parking_lot::Mutex::new(TaskQueue::new());
    let runner_notifier = Arc::new(runner_notifier);
    Self {
      collab_service,
      collab_db,
      queue,
      runner_notifier,
    }
  }
}

#[async_trait]
impl TaskHandler<BlockTask> for BlockTaskHandler {
  async fn prepare_task(&self) -> Option<PendingTask<BlockTask>> {
    let mut queue = self.queue.try_lock()?;
    let task = queue.pop()?;
    let collab_db = self.collab_db.upgrade()?;

    // The tasks that get the row with given row_id might be duplicated, so we need to check if the
    // task is already done.
    let is_exist = match &task.payload {
      BlockTask::FetchRow { uid, row_id, .. } => {
        collab_db.read_txn().is_exist(*uid, row_id.as_ref())
      },
      BlockTask::BatchFetchRow { uid, row_ids, .. } => match row_ids.first() {
        None => true,
        Some(row_id) => collab_db.read_txn().is_exist(*uid, row_id.as_ref()),
      },
    };

    return if is_exist { None } else { Some(task) };
  }

  async fn handle_task(&self, mut task: PendingTask<BlockTask>) -> Option<()> {
    trace!("handle task: {:?}", task);
    task.set_state(TaskState::Processing);
    let collab_db = self.collab_db.clone();
    match &task.payload {
      BlockTask::FetchRow {
        row_id,
        uid,
        sender,
        ..
      } => {
        if let Some(collab_service) = self.collab_service.upgrade() {
          trace!("fetching database row: {:?}", row_id);
          if let Ok(doc_state) = collab_service
            .get_collab_doc_state(row_id.as_ref(), CollabType::DatabaseRow)
            .await
          {
            let collab = collab_service.build_collab_with_config(
              *uid,
              row_id,
              CollabType::DatabaseRow,
              collab_db,
              doc_state,
              CollabPersistenceConfig::default(),
            );

            let _ = sender.send(collab).await;
          }
        }
      },
      BlockTask::BatchFetchRow {
        row_ids,
        uid,
        sender,
        ..
      } => {
        if let Some(collab_service) = self.collab_service.upgrade() {
          trace!("batch fetching database row");
          let object_ids = row_ids.iter().map(|id| id.to_string()).collect::<Vec<_>>();
          if let Ok(updates_by_oid) = collab_service
            .batch_get_collab_update(object_ids, CollabType::DatabaseRow)
            .await
          {
            let mut collabs = vec![];
            for (oid, doc_state) in updates_by_oid {
              let collab = collab_service.build_collab_with_config(
                *uid,
                &oid,
                CollabType::DatabaseRow,
                collab_db.clone(),
                doc_state,
                CollabPersistenceConfig::default(),
              );
              collabs.push((oid, collab));
              yield_now().await;
            }
            let _ = sender.send(collabs).await;
          }
        }
      },
    }
    task.set_state(TaskState::Done);
    Some(())
  }

  fn notify(&self) {
    let _ = self.runner_notifier.send(false);
  }
}

#[allow(dead_code)]
fn save_row(
  collab_db: &Arc<CollabKVDB>,
  collab_doc_state: DocStateSource,
  uid: i64,
  row_id: &RowId,
) -> Option<RowDetail> {
  if collab_doc_state.is_empty() {
    tracing::error!("Unexpected empty row: {} collab update", row_id.as_ref());
    return None;
  }
  let row = collab_db.with_write_txn(|write_txn| {
    match MutexCollab::new_with_doc_state(
      CollabOrigin::Empty,
      row_id.as_ref(),
      collab_doc_state,
      vec![],
      false,
    ) {
      Ok(collab) => {
        let collab_lock_guard = collab.lock();
        let encode_collab = collab_lock_guard.encode_collab_v1();
        let object_id = row_id.as_ref();
        if let Err(e) = write_txn.flush_doc(
          uid,
          object_id,
          encode_collab.state_vector.to_vec(),
          encode_collab.doc_state.to_vec(),
        ) {
          tracing::error!(
            "{} failed to save the database row collab: {:?}",
            row_id.as_ref(),
            e
          );
        }

        let txn = collab_lock_guard.transact();
        let row_detail = RowDetail::from_collab(&collab_lock_guard, &txn);
        if row_detail.is_none() {
          tracing::error!("{} doesn't have any row information in it", row_id.as_ref());
        }
        Ok(row_detail)
      },

      Err(e) => {
        tracing::error!("Failed to deserialize doc state to row: {:?}", e);
        Ok(None)
      },
    }
  });

  match row {
    Ok(None) => None,
    Ok(row) => row,
    Err(e) => {
      tracing::error!(
        "{} failed to save the database row collab: {:?}",
        row_id.as_ref(),
        e
      );
      None
    },
  }
}

pub type FetchRowSender = tokio::sync::mpsc::Sender<Arc<MutexCollab>>;
pub type BatchFetchRowSender = tokio::sync::mpsc::Sender<Vec<(String, Arc<MutexCollab>)>>;

#[derive(Clone)]
pub enum BlockTask {
  FetchRow {
    uid: i64,
    row_id: RowId,
    seq: u32,
    sender: FetchRowSender,
  },
  BatchFetchRow {
    uid: i64,
    row_ids: Vec<RowId>,
    seq: u32,
    sender: BatchFetchRowSender,
  },
}

impl Debug for BlockTask {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    match self {
      BlockTask::FetchRow { .. } => f.write_fmt(format_args!("Fetch database row")),
      BlockTask::BatchFetchRow { .. } => f.write_fmt(format_args!("Batch fetch database row")),
    }
  }
}

impl Ord for BlockTask {
  fn cmp(&self, other: &Self) -> Ordering {
    match (self, other) {
      (BlockTask::BatchFetchRow { seq: seq1, .. }, BlockTask::BatchFetchRow { seq: seq2, .. }) => {
        seq1.cmp(seq2).reverse()
      },
      (BlockTask::BatchFetchRow { .. }, BlockTask::FetchRow { .. }) => Ordering::Greater,
      (BlockTask::FetchRow { .. }, BlockTask::BatchFetchRow { .. }) => Ordering::Less,
      (BlockTask::FetchRow { seq: seq1, .. }, BlockTask::FetchRow { seq: seq2, .. }) => {
        seq1.cmp(seq2).reverse()
      },
    }
  }
}

impl Eq for BlockTask {}

impl PartialEq<Self> for BlockTask {
  fn eq(&self, other: &Self) -> bool {
    match (self, other) {
      (BlockTask::BatchFetchRow { seq: seq1, .. }, BlockTask::BatchFetchRow { seq: seq2, .. }) => {
        seq1 == seq2
      },
      (BlockTask::BatchFetchRow { .. }, BlockTask::FetchRow { .. }) => false,
      (BlockTask::FetchRow { .. }, BlockTask::BatchFetchRow { .. }) => false,
      (BlockTask::FetchRow { seq: seq1, .. }, BlockTask::FetchRow { seq: seq2, .. }) => {
        seq1 == seq2
      },
    }
  }
}

impl PartialOrd<Self> for BlockTask {
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
    Some(self.cmp(other))
  }
}

impl RequestPayload for BlockTask {}
