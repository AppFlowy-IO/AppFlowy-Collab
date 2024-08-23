use dashmap::DashMap;

use collab_entity::CollabType;

use crate::blocks::task_controller::{BlockTask, BlockTaskController};
use crate::error::DatabaseError;
use crate::rows::{
  meta_id_from_row_id, Cell, DatabaseRow, Row, RowChangeSender, RowDetail, RowId, RowMeta,
  RowMetaKey, RowMetaUpdate, RowUpdate,
};
use crate::views::RowOrder;
use crate::workspace_database::{CollabPersistenceImpl, DatabaseCollabService};

use collab::preclude::Collab;

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use tokio::sync::broadcast::Sender;
use tokio::sync::{broadcast, RwLock};
use tracing::{error, info, instrument, trace, warn};
use uuid::Uuid;

#[derive(Clone, Debug)]
pub enum BlockEvent {
  /// The Row is fetched from the remote.
  DidFetchRow(Vec<RowDetail>),
}

/// Each [Block] contains a list of [DatabaseRow]s. Each [DatabaseRow] represents a row in the database.
/// Currently, we only use one [Block] to manage all the rows in the database. In the future, we
/// might want to split the rows into multiple [Block]s to improve performance.
#[derive(Clone)]
pub struct Block {
  database_id: String,
  collab_service: Arc<dyn DatabaseCollabService>,
  task_controller: Arc<BlockTaskController>,
  sequence: Arc<AtomicU32>,
  pub row_mem_cache: Arc<DashMap<RowId, Arc<RwLock<DatabaseRow>>>>,
  pub notifier: Arc<Sender<BlockEvent>>,
  row_change_tx: RowChangeSender,
}

impl Block {
  pub fn new(
    database_id: String,
    collab_service: Arc<dyn DatabaseCollabService>,
    row_change_tx: RowChangeSender,
  ) -> Block {
    let controller = BlockTaskController::new(Arc::downgrade(&collab_service));
    let task_controller = Arc::new(controller);
    let (notifier, _) = broadcast::channel(1000);
    Self {
      database_id,
      task_controller,
      collab_service,
      sequence: Arc::new(Default::default()),
      row_mem_cache: Arc::new(Default::default()),
      notifier: Arc::new(notifier),
      row_change_tx,
    }
  }

  pub fn subscribe_event(&self) -> broadcast::Receiver<BlockEvent> {
    self.notifier.subscribe()
  }

  pub async fn batch_load_rows(&self, row_ids: Vec<RowId>) -> Result<(), DatabaseError> {
    let (rows_on_disk, rows_not_on_disk) = match self.collab_service.persistence() {
      None => (vec![], row_ids),
      Some(persistence) => persistence.is_row_exist_partition(row_ids),
    };

    info!(
      "batch_load_rows: rows_on_disk: {}, rows_not_on_disk: {}",
      rows_on_disk.len(),
      rows_not_on_disk.len()
    );

    let cloned_notifier = self.notifier.clone();
    let row_details = rows_on_disk
      .into_iter()
      .filter_map(|row_id| {
        let collab = self.create_collab_for_row(&row_id).ok()?;
        let row_collab = DatabaseRow::new(
          row_id.clone(),
          collab,
          self.row_change_tx.clone(),
          None,
          self.collab_service.clone(),
        );
        let row_detail = RowDetail::from_collab(&row_collab)?;
        self
          .row_mem_cache
          .insert(row_id.clone(), Arc::new(RwLock::new(row_collab)));
        Some(row_detail)
      })
      .collect::<Vec<RowDetail>>();

    if !row_details.is_empty() {
      let _ = cloned_notifier.send(BlockEvent::DidFetchRow(row_details));
    }
    self.batch_load_rows_from_remote(rows_not_on_disk);
    Ok(())
  }

  fn batch_load_rows_from_remote(&self, row_ids: Vec<RowId>) {
    // start loading rows that not on disk
    info!("batch load {} rows from remote", row_ids.len());
    let (tx, mut rx) = tokio::sync::mpsc::channel(100);
    self.task_controller.add_task(BlockTask::BatchFetchRow {
      row_ids,
      seq: self.sequence.fetch_add(1, Ordering::SeqCst),
      sender: tx,
    });

    let row_change_tx = self.row_change_tx.clone();
    let row_mem_cache = self.row_mem_cache.clone();
    let notifier = self.notifier.clone();
    let collab_service = self.collab_service.clone();
    tokio::spawn(async move {
      let mut success_row_count = 0;
      let mut fail_row_count = 0;
      while let Some(row_collabs) = rx.recv().await {
        for (row_id, row_collab) in row_collabs {
          match row_collab {
            Ok(row_collab) => {
              let row_id = RowId::from(row_id);
              let row_detail = RowDetail::from_collab(&row_collab);
              let row = Arc::new(RwLock::new(DatabaseRow::new(
                row_id.clone(),
                row_collab,
                row_change_tx.clone(),
                None,
                collab_service.clone(),
              )));
              row_mem_cache.insert(row_id, row);
              success_row_count += 1;
              if let Some(row_detail) = row_detail {
                let _ = notifier.send(BlockEvent::DidFetchRow(vec![row_detail]));
              }
            },
            Err(err) => {
              fail_row_count += 1;
              error!("Can't fetch the row from remote: {:?}", err);
            },
          }
        }
      }

      info!(
        "did load rows from remote: success_row_count: {}, fail_row_count: {}",
        success_row_count, fail_row_count
      );
    });
  }

  pub fn create_rows<T>(&self, rows: Vec<T>) -> Vec<RowOrder>
  where
    T: Into<Row> + Send,
  {
    let mut row_orders = Vec::with_capacity(rows.len());
    for row in rows {
      if let Ok(row_order) = self.create_row(row) {
        row_orders.push(row_order);
      }
    }
    row_orders
  }

  pub fn create_row<T: Into<Row>>(&self, row: T) -> Result<RowOrder, DatabaseError> {
    let row = row.into();
    let row_id = row.id.clone();
    let row_order = RowOrder {
      id: row.id.clone(),
      height: row.height,
    };

    trace!("create_row: {}", row_id);
    if let Some(persistence) = self.collab_service.persistence() {
      if persistence.is_collab_exist(&row_id) {
        warn!("The row already exists: {:?}", row_id);
        return Err(DatabaseError::RecordAlreadyExist);
      }
    }

    let collab = self.create_collab_for_row(&row_id)?;
    let database_row = DatabaseRow::new(
      row_id.clone(),
      collab,
      self.row_change_tx.clone(),
      Some(row),
      self.collab_service.clone(),
    );

    database_row.write_to_disk()?;
    let database_row = Arc::new(RwLock::new(database_row));
    self.row_mem_cache.insert(row_id, database_row);

    Ok(row_order)
  }

  pub fn get_row(&self, row_id: &RowId) -> Option<Arc<RwLock<DatabaseRow>>> {
    self
      .row_mem_cache
      .get(row_id)
      .map(|row| Some(row.value().clone()))?
  }

  pub async fn get_row_meta(&self, row_id: &RowId) -> Option<RowMeta> {
    let database_row = self.get_row(row_id)?;
    let read_guard = database_row.read().await;
    read_guard.get_row_meta()
  }

  pub async fn get_cell(&self, row_id: &RowId, field_id: &str) -> Option<Cell> {
    let database_row = self.get_row(row_id)?;
    let read_guard = database_row.read().await;
    read_guard.get_cell(field_id)
  }

  pub fn get_row_document_id(&self, row_id: &RowId) -> Option<String> {
    let row_id = Uuid::parse_str(row_id).ok()?;
    Some(meta_id_from_row_id(&row_id, RowMetaKey::DocumentId))
  }

  /// If the row with given id not exist. It will return an empty row with given id.
  /// An empty [Row] is a row with no cells.
  ///
  #[instrument(level = "debug", skip_all)]
  pub async fn get_rows_from_row_orders(&self, row_orders: &[RowOrder]) -> Vec<Row> {
    let mut rows = Vec::new();
    for row_order in row_orders {
      let row = match self.get_or_init_row(row_order.id.clone()) {
        Err(_) => Row::empty(row_order.id.clone(), &self.database_id),
        Ok(database_row) => database_row
          .read()
          .await
          .get_row()
          .unwrap_or_else(|| Row::empty(row_order.id.clone(), &self.database_id)),
      };

      rows.push(row);
    }
    rows
  }

  pub fn delete_row(&self, row_id: &RowId) -> Option<Arc<RwLock<DatabaseRow>>> {
    let row = self.row_mem_cache.remove(row_id).map(|(_, row)| row);
    if let Some(persistence) = self.collab_service.persistence() {
      if let Err(err) = persistence.delete_collab(row_id) {
        error!("Can't delete the row from disk: {:?}", err);
      }
    }
    row
  }

  pub async fn update_row<F>(&mut self, row_id: RowId, f: F)
  where
    F: FnOnce(RowUpdate),
  {
    match self.get_row(&row_id) {
      None => {
        error!(
          "fail to update row. the database row is not created: {:?}",
          row_id
        )
      },
      Some(database_row) => {
        database_row.write().await.update::<F>(f);
      },
    }
  }

  pub async fn update_row_meta<F>(&mut self, row_id: &RowId, f: F)
  where
    F: FnOnce(RowMetaUpdate),
  {
    let row = self.row_mem_cache.get(row_id);
    match row {
      None => {
        trace!(
          "fail to update row meta. the row is not in the cache: {:?}",
          row_id
        )
      },
      Some(row) => {
        row.write().await.update_meta::<F>(f);
      },
    }
  }

  /// Get the [DatabaseRow] from the cache. If the row is not in the cache, initialize it.
  #[instrument(level = "debug", skip_all)]
  pub fn get_or_init_row(&self, row_id: RowId) -> Result<Arc<RwLock<DatabaseRow>>, DatabaseError> {
    self
      .row_mem_cache
      .entry(row_id.clone())
      .or_try_insert_with(|| self.create_row_instance(row_id))
      .map(|r| r.value().clone())
  }

  pub fn create_row_instance(
    &self,
    row_id: RowId,
  ) -> Result<Arc<RwLock<DatabaseRow>>, DatabaseError> {
    let exists = match self.collab_service.persistence() {
      None => false,
      Some(persistence) => persistence.is_collab_exist(&row_id),
    };

    if exists {
      trace!("create row instance from disk: {:?}", row_id);
      let collab = self.create_collab_for_row(&row_id)?;
      let database_row = Arc::new(RwLock::new(DatabaseRow::new(
        row_id.clone(),
        collab,
        self.row_change_tx.clone(),
        None,
        self.collab_service.clone(),
      )));
      return Ok(database_row);
    }

    // Can't find the row in local disk, fetch it from remote.
    trace!(
      "Row:{:?} not found in local disk, fetch it from remote",
      row_id
    );
    let (sender, mut rx) = tokio::sync::mpsc::channel(1);
    self.task_controller.add_task(BlockTask::FetchRow {
      row_id: row_id.clone(),
      seq: self.sequence.fetch_add(1, Ordering::SeqCst),
      sender,
    });

    let weak_notifier = Arc::downgrade(&self.notifier);
    let change_tx = self.row_change_tx.clone();
    let row_cache = self.row_mem_cache.clone();
    let cloned_row_id = row_id.clone();
    let cloned_collab_service = self.collab_service.clone();
    tokio::spawn(async move {
      if let Some(Ok(row_collab)) = rx.recv().await {
        let row_detail = RowDetail::from_collab(&row_collab);
        trace!("create row instance from remote: {:?}", cloned_row_id);
        let row = Arc::new(RwLock::new(DatabaseRow::new(
          cloned_row_id.clone(),
          row_collab,
          change_tx,
          None,
          cloned_collab_service,
        )));
        row_cache.insert(cloned_row_id, row);
        row_detail.map(|row_detail| {
          weak_notifier.upgrade().map(|notifier| {
            let _ = notifier.send(BlockEvent::DidFetchRow(vec![row_detail]));
          })
        });
      } else {
        error!("Can't fetch the row from remote: {:?}", cloned_row_id);
      }
    });
    Err(DatabaseError::DatabaseRowNotFound {
      row_id,
      reason: "the row is not exist in local disk".to_string(),
    })
  }

  fn create_collab_for_row(&self, row_id: &RowId) -> Result<Collab, DatabaseError> {
    let data_source = CollabPersistenceImpl {
      persistence: self.collab_service.persistence(),
    };
    self
      .collab_service
      .build_collab(row_id, CollabType::DatabaseRow, data_source.into())
  }
}
