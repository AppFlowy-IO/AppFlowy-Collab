use std::cmp::Ordering;
use std::fmt::{Debug, Formatter};
use std::sync::{Arc, Weak};

use crate::error::DatabaseError;
use crate::rows::RowId;
use crate::workspace_database::{
  CollabPersistenceImpl, DatabaseCollabPersistenceService, DatabaseCollabService,
};
use collab::core::collab::DataSource;

use collab::preclude::Collab;
use collab_entity::CollabType;

use collab::entity::EncodedCollab;

use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tokio::task::{yield_now, JoinHandle};
use tracing::{error, trace};

/// A [BlockTaskController] is used to control how the [BlockTask]s are executed.
/// It contains a [TaskQueue] to queue the [BlockTask]s and a [TaskHandler] to handle the
/// [BlockTask]s.
///
pub struct BlockTaskController {
  sender: UnboundedSender<BlockTask>,
  processor: JoinHandle<()>,
}

impl BlockTaskController {
  pub fn new(collab_service: Weak<dyn DatabaseCollabService>) -> Self {
    let (sender, receiver) = unbounded_channel();
    let processor = tokio::spawn(Self::run(receiver, collab_service));

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
    collab_service: Weak<dyn DatabaseCollabService>,
  ) {
    while let Some(task) = receiver.recv().await {
      if let Some(collab_service) = collab_service.upgrade() {
        if !Self::redundant(collab_service.persistence(), &task) {
          if let Err(err) = Self::handle_task(task, collab_service).await {
            tracing::error!("Failed to handle task: {}", err);
          }
        }
      } else {
        break; // collab_service is dropped
      }
    }
  }

  async fn handle_task(
    task: BlockTask,
    collab_service: Arc<dyn DatabaseCollabService>,
  ) -> anyhow::Result<()> {
    trace!("handle task: {:?}", task);
    match &task {
      BlockTask::FetchRow { row_id, sender, .. } => {
        trace!("fetching database row: {:?}", row_id);
        if let Ok(encode_collab) = collab_service
          .get_encode_collab(row_id.as_ref(), CollabType::DatabaseRow)
          .await
        {
          if let Some(encode_collab) = encode_collab.clone() {
            write_encode_collab_to_disk(&collab_service, encode_collab, row_id.as_str());
          }

          let data_source = encode_collab.map(DataSource::from).unwrap_or_else(|| {
            CollabPersistenceImpl {
              persistence: collab_service.persistence(),
            }
            .into()
          });

          let collab = collab_service.build_collab(row_id, CollabType::DatabaseRow, data_source);

          let _ = sender.send(collab).await;
        }
      },
      BlockTask::BatchFetchRow {
        row_ids, sender, ..
      } => {
        trace!("batch fetching database row");
        let object_ids = row_ids.iter().map(|id| id.to_string()).collect::<Vec<_>>();
        if let Ok(updates_by_oid) = collab_service
          .batch_get_encode_collab(object_ids, CollabType::DatabaseRow)
          .await
        {
          let mut collabs = vec![];
          let cloned_updates_by_oid = updates_by_oid.clone();
          let cloned_collab_service = collab_service.clone();
          let _ = tokio::task::spawn_blocking(move || {
            for (oid, encode_collab) in cloned_updates_by_oid {
              write_encode_collab_to_disk(&cloned_collab_service, encode_collab, &oid);
            }
          })
          .await;

          for (oid, encode_collab) in updates_by_oid {
            let collab = collab_service.build_collab(
              &oid,
              CollabType::DatabaseRow,
              DataSource::from(encode_collab),
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
  fn redundant(
    collab_persistence: Option<Box<dyn DatabaseCollabPersistenceService>>,
    task: &BlockTask,
  ) -> bool {
    match collab_persistence {
      None => false,
      Some(collab_persistence) => {
        let redundant = match &task {
          BlockTask::FetchRow { row_id, .. } => collab_persistence.is_collab_exist(row_id.as_ref()),
          BlockTask::BatchFetchRow { row_ids, .. } => match row_ids.first() {
            None => true,
            Some(row_id) => collab_persistence.is_collab_exist(row_id.as_ref()),
          },
        };
        redundant
      },
    }
  }
}

impl Drop for BlockTaskController {
  fn drop(&mut self) {
    self.processor.abort();
  }
}

fn write_encode_collab_to_disk(
  collab_service: &Arc<dyn DatabaseCollabService>,
  encode_collab: EncodedCollab,
  object_id: &str,
) {
  match collab_service.persistence() {
    None => {
      trace!("No persistence service found, skip writing collab to disk");
    },
    Some(persistence) => {
      if let Err(err) = persistence.flush_collab(object_id, encode_collab) {
        error!(
          "{} failed to save the database row collab: {:?}",
          object_id, err
        );
      }
    },
  }
}

pub type FetchRowSender = tokio::sync::mpsc::Sender<Result<Collab, DatabaseError>>;
pub type BatchFetchRowSender =
  tokio::sync::mpsc::Sender<Vec<(String, Result<Collab, DatabaseError>)>>;

#[derive(Clone)]
pub enum BlockTask {
  FetchRow {
    row_id: RowId,
    seq: u32,
    sender: FetchRowSender,
  },
  BatchFetchRow {
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
