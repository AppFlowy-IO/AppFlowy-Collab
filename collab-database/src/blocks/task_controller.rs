use std::cmp::Ordering;
use std::fmt::{Debug, Formatter};
use std::sync::{Arc, Weak};

use crate::error::DatabaseError;
use crate::rows::{RowDetail, RowId};
use crate::workspace_database::DatabaseCollabService;
use collab::core::collab::DataSource;
use collab::core::origin::CollabOrigin;
use collab::preclude::Collab;
use collab_entity::CollabType;
use collab_plugins::local_storage::kv::doc::CollabKVAction;
use collab_plugins::local_storage::kv::{KVTransactionDB, PersistenceError};
use collab_plugins::local_storage::rocksdb::util::KVDBCollabPersistenceImpl;

use collab_plugins::CollabKVDB;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tokio::task::{yield_now, JoinHandle};
use tracing::trace;

/// A [BlockTaskController] is used to control how the [BlockTask]s are executed.
/// It contains a [TaskQueue] to queue the [BlockTask]s and a [TaskHandler] to handle the
/// [BlockTask]s.
///
pub struct BlockTaskController {
  sender: UnboundedSender<BlockTask>,
  processor: JoinHandle<()>,
}

impl BlockTaskController {
  pub fn new(collab_db: Weak<CollabKVDB>, collab_service: Weak<dyn DatabaseCollabService>) -> Self {
    let (sender, receiver) = unbounded_channel();
    let processor = tokio::spawn(Self::run(receiver, collab_db, collab_service));

    Self { sender, processor }
  }

  /// Add a new task to the queue. The task with higher sequence number will be executed first.
  /// Just like Last In First Out (LIFO).
  pub fn add_task(&self, task: BlockTask) {
    if self.sender.send(task).is_err() {
      tracing::error!("Cannot schedule task - processing loop has been closed");
    }
  }
  async fn run(
    mut receiver: UnboundedReceiver<BlockTask>,
    collab_db: Weak<CollabKVDB>,
    collab_service: Weak<dyn DatabaseCollabService>,
  ) {
    while let Some(task) = receiver.recv().await {
      if let Some(collab_db) = collab_db.upgrade() {
        if let Some(collab_service) = collab_service.upgrade() {
          if !Self::redundant(&collab_db, &task) {
            if let Err(err) = Self::handle_task(task, collab_db, collab_service).await {
              tracing::error!("Failed to handle task: {}", err);
            }
          }
        } else {
          break; // collab_service is dropped
        }
      } else {
        break; // collab_db is dropped
      }
    }
  }

  async fn handle_task(
    task: BlockTask,
    collab_db: Arc<CollabKVDB>,
    collab_service: Arc<dyn DatabaseCollabService>,
  ) -> anyhow::Result<()> {
    trace!("handle task: {:?}", task);
    match &task {
      BlockTask::FetchRow {
        row_id,
        uid,
        sender,
        ..
      } => {
        trace!("fetching database row: {:?}", row_id);
        if let Ok(doc_state) = collab_service
          .get_collab_doc_state(row_id.as_ref(), CollabType::DatabaseRow)
          .await
        {
          let data_source = doc_state.unwrap_or_else(|| {
            KVDBCollabPersistenceImpl {
              db: Arc::downgrade(&collab_db),
              uid: *uid,
            }
            .into()
          });

          let collab = collab_service.build_collab(
            *uid,
            row_id,
            CollabType::DatabaseRow,
            Arc::downgrade(&collab_db),
            data_source,
          );

          let _ = sender.send(collab).await;
        }
      },
      BlockTask::BatchFetchRow {
        row_ids,
        uid,
        sender,
        ..
      } => {
        trace!("batch fetching database row");
        let object_ids = row_ids.iter().map(|id| id.to_string()).collect::<Vec<_>>();
        if let Ok(updates_by_oid) = collab_service
          .batch_get_collab_update(object_ids, CollabType::DatabaseRow)
          .await
        {
          let mut collabs = vec![];
          for (oid, doc_state) in updates_by_oid {
            let collab = collab_service.build_collab(
              *uid,
              &oid,
              CollabType::DatabaseRow,
              Arc::downgrade(&collab_db),
              doc_state,
            );
            collabs.push((oid, collab));
            yield_now().await;
          }
          let _ = sender.send(collabs).await;
        }
      },
    }
    Ok(())
  }

  /// The tasks that get the row with given row_id might be duplicated, so we need to check if the
  /// task is already done.
  fn redundant(collab_db: &CollabKVDB, task: &BlockTask) -> bool {
    let redundant = match &task {
      BlockTask::FetchRow { uid, row_id, .. } => {
        collab_db.read_txn().is_exist(*uid, row_id.as_ref())
      },
      BlockTask::BatchFetchRow { uid, row_ids, .. } => match row_ids.first() {
        None => true,
        Some(row_id) => collab_db.read_txn().is_exist(*uid, row_id.as_ref()),
      },
    };
    redundant
  }
}

impl Drop for BlockTaskController {
  fn drop(&mut self) {
    self.processor.abort();
  }
}

#[allow(dead_code)]
fn save_row(
  collab_db: &Arc<CollabKVDB>,
  collab_doc_state: DataSource,
  uid: i64,
  row_id: &RowId,
) -> Option<RowDetail> {
  if collab_doc_state.is_empty() {
    tracing::error!("Unexpected empty row: {} collab update", row_id.as_ref());
    return None;
  }
  let row = collab_db.with_write_txn(|write_txn| {
    match Collab::new_with_source(
      CollabOrigin::Empty,
      row_id.as_ref(),
      collab_doc_state,
      vec![],
      false,
    ) {
      Ok(collab) => {
        let encode_collab = collab
          .encode_collab_v1(|collab| {
            CollabType::DatabaseRow
              .validate_require_data(collab)
              .map_err(|_| DatabaseError::NoRequiredData)
          })
          .map_err(|err| PersistenceError::Internal(err.into()))?;
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

        let row_detail = RowDetail::from_collab(&collab);
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

pub type FetchRowSender = tokio::sync::mpsc::Sender<Result<Collab, DatabaseError>>;
pub type BatchFetchRowSender =
  tokio::sync::mpsc::Sender<Vec<(String, Result<Collab, DatabaseError>)>>;

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
