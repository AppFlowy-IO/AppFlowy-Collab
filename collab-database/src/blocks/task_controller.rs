use std::cmp::Ordering;
use std::fmt::{Debug, Formatter};
use std::sync::{Arc, Weak};

use async_trait::async_trait;
use collab::core::collab::{CollabRawData, MutexCollab};
use collab::core::origin::CollabOrigin;
use collab_entity::CollabType;
use collab_persistence::doc::YrsDocAction;
use collab_persistence::kv::rocks_kv::RocksCollabDB;
use tokio::sync::watch;

use crate::blocks::queue::{
  PendingTask, RequestPayload, TaskHandler, TaskQueue, TaskQueueRunner, TaskState,
};
use crate::rows::{RowDetail, RowId};
use crate::user::DatabaseCollabService;

/// A [BlockTaskController] is used to control how the [BlockTask]s are executed.
/// It contains a [TaskQueue] to queue the [BlockTask]s and a [TaskHandler] to handle the
/// [BlockTask]s.
///
pub struct BlockTaskController {
  request_handler: Arc<BlockTaskHandler>,
}

impl BlockTaskController {
  pub fn new(
    collab_db: Weak<RocksCollabDB>,
    collab_service: Weak<dyn DatabaseCollabService>,
  ) -> Self {
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
  collab_db: Weak<RocksCollabDB>,
  collab_service: Weak<dyn DatabaseCollabService>,
  queue: parking_lot::Mutex<TaskQueue<BlockTask>>,
  runner_notifier: Arc<watch::Sender<bool>>,
}

impl BlockTaskHandler {
  pub fn new(
    collab_service: Weak<dyn DatabaseCollabService>,
    collab_db: Weak<RocksCollabDB>,
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
    task.set_state(TaskState::Processing);
    let collab_service = self.collab_service.upgrade()?;
    let collab_db = self.collab_db.upgrade()?;
    match &task.payload {
      BlockTask::FetchRow {
        row_id,
        uid,
        sender,
        ..
      } => {
        tracing::trace!("fetching database row: {:?}", row_id);
        if let Ok(updates) = collab_service
          .get_collab_update(row_id.as_ref(), CollabType::DatabaseRow)
          .await
        {
          if let Some(row_detail) = save_row(&collab_db, updates, *uid, row_id) {
            let _ = sender.send(row_detail).await;
          }
        }
      },
      BlockTask::BatchFetchRow {
        row_ids,
        uid,
        sender,
        ..
      } => {
        tracing::trace!("batch fetching database row");
        let object_ids = row_ids.iter().map(|id| id.to_string()).collect::<Vec<_>>();
        if let Ok(updates_by_oid) = collab_service
          .batch_get_collab_update(object_ids, CollabType::DatabaseRow)
          .await
        {
          let mut row_details = vec![];
          for (oid, updates) in updates_by_oid {
            if let Some(row_detail) = save_row(&collab_db, updates, *uid, oid) {
              row_details.push(row_detail);
            }
          }

          let _ = sender.send(row_details).await;
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

fn save_row<R: AsRef<str>>(
  collab_db: &Arc<RocksCollabDB>,
  collab_raw_data: CollabRawData,
  uid: i64,
  row_id: R,
) -> Option<RowDetail> {
  if collab_raw_data.is_empty() {
    tracing::error!("Unexpected empty row: {} collab update", row_id.as_ref());
    return None;
  }
  let row = collab_db.with_write_txn(|write_txn| {
    match MutexCollab::new_with_raw_data(
      CollabOrigin::Empty,
      row_id.as_ref(),
      collab_raw_data,
      vec![],
    ) {
      Ok(collab) => {
        let collab_guard = collab.lock();
        let txn = collab_guard.transact();
        let object_id = row_id.as_ref();
        if let Err(e) = write_txn.create_new_doc(uid, object_id, &txn) {
          tracing::error!("Failed to save the database row collab: {:?}", e);
        }
        Ok(RowDetail::from_collab(&collab_guard, &txn))
      },

      Err(e) => {
        tracing::error!("Failed to create database row collab: {:?}", e);
        Ok(None)
      },
    }
  });

  match row {
    Ok(None) => {
      tracing::error!("Unexpected empty row. The row should not be empty at this point.");
      None
    },
    Ok(row) => row,
    Err(e) => {
      tracing::error!("Failed to save the database row collab: {:?}", e);
      None
    },
  }
}

pub type FetchRowSender = tokio::sync::mpsc::Sender<RowDetail>;
pub type BatchFetchRowSender = tokio::sync::mpsc::Sender<Vec<RowDetail>>;

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
