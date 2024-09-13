use dashmap::DashMap;

use collab_entity::CollabType;

use crate::error::DatabaseError;
use crate::rows::{
  default_database_row_data, meta_id_from_row_id, Cell, DatabaseRow, Row, RowChangeSender,
  RowDetail, RowId, RowMeta, RowMetaKey, RowMetaUpdate, RowUpdate,
};
use crate::views::RowOrder;
use crate::workspace_database::DatabaseCollabService;

use collab::preclude::Collab;

use collab::entity::EncodedCollab;
use collab::lock::RwLock;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio::sync::broadcast::Sender;
use tracing::{error, instrument, trace, warn};
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
  // task_controller: Arc<BlockTaskController>,
  // sequence: Arc<AtomicU32>,
  pub row_mem_cache: Arc<DashMap<RowId, Arc<RwLock<DatabaseRow>>>>,
  pub notifier: Arc<Sender<BlockEvent>>,
  row_change_tx: Option<RowChangeSender>,
}

impl Block {
  pub fn new(
    database_id: String,
    collab_service: Arc<dyn DatabaseCollabService>,
    row_change_tx: Option<RowChangeSender>,
  ) -> Block {
    let (notifier, _) = broadcast::channel(1000);
    Self {
      database_id,
      collab_service,
      row_mem_cache: Arc::new(Default::default()),
      notifier: Arc::new(notifier),
      row_change_tx,
    }
  }

  pub fn subscribe_event(&self) -> broadcast::Receiver<BlockEvent> {
    self.notifier.subscribe()
  }

  pub async fn batch_load_rows(&self, row_ids: Vec<RowId>) -> Result<(), DatabaseError> {
    let cloned_notifier = self.notifier.clone();
    let mut row_on_disk_details = vec![];
    for row_id in row_ids.into_iter() {
      let collab = self.create_collab_for_row(&row_id, None).await?;
      match DatabaseRow::open(
        row_id.clone(),
        collab,
        self.row_change_tx.clone(),
        self.collab_service.clone(),
      ) {
        Ok(row_collab) => {
          if let Some(row_detail) = RowDetail::from_collab(&row_collab) {
            self
              .row_mem_cache
              .insert(row_id.clone(), Arc::new(RwLock::from(row_collab)));
            row_on_disk_details.push(row_detail);
          }
        },
        Err(err) => {
          error!("fail to load row: {:?}", err);
        },
      }
    }

    if !row_on_disk_details.is_empty() {
      let _ = cloned_notifier.send(BlockEvent::DidFetchRow(row_on_disk_details));
    }
    Ok(())
  }

  pub async fn create_rows<T>(&self, rows: Vec<T>) -> Vec<RowOrder>
  where
    T: Into<Row> + Send,
  {
    let mut row_orders = Vec::with_capacity(rows.len());
    for row in rows {
      if let Ok(row_order) = self.create_new_row(row).await {
        row_orders.push(row_order);
      }
    }
    row_orders
  }

  pub async fn create_new_row<T: Into<Row>>(&self, row: T) -> Result<RowOrder, DatabaseError> {
    let row = row.into();
    let row_id = row.id.clone();
    let row_order = RowOrder {
      id: row.id.clone(),
      height: row.height,
    };

    trace!("create new row: {}", row_id);
    if let Some(persistence) = self.collab_service.persistence() {
      if persistence.is_collab_exist(&row_id) {
        warn!("The row already exists: {:?}", row_id);
        return Err(DatabaseError::RecordAlreadyExist);
      }
    }

    let encoded_collab = default_database_row_data(&row_id, row);
    let collab = self
      .create_collab_for_row(&row_id, Some(encoded_collab))
      .await?;
    let database_row = DatabaseRow::open(
      row_id.clone(),
      collab,
      self.row_change_tx.clone(),
      self.collab_service.clone(),
    )?;

    let database_row = Arc::new(RwLock::from(database_row));
    self.row_mem_cache.insert(row_id, database_row);
    Ok(row_order)
  }

  pub async fn get_database_row(&self, row_id: &RowId) -> Option<Arc<RwLock<DatabaseRow>>> {
    self
      .row_mem_cache
      .get(row_id)
      .map(|entry| entry.value().clone())
  }

  pub async fn get_row_meta(&self, row_id: &RowId) -> Option<RowMeta> {
    let database_row = self.get_database_row(row_id).await?;
    let read_guard = database_row.read().await;
    read_guard.get_row_meta()
  }

  pub async fn get_cell(&self, row_id: &RowId, field_id: &str) -> Option<Cell> {
    let database_row = self.get_database_row(row_id).await?;
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
      let row = match self.get_or_init_database_row(&row_order.id).await {
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
    match self.get_database_row(&row_id).await {
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
    let database_row = self.row_mem_cache.get(row_id);
    match database_row {
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
  pub async fn get_or_init_database_row(
    &self,
    row_id: &RowId,
  ) -> Result<Arc<RwLock<DatabaseRow>>, DatabaseError> {
    let value = self
      .row_mem_cache
      .get(row_id)
      .map(|entry| entry.value().clone());

    match value {
      None => self.init_row_instance(row_id.clone()).await.map_err(|_| {
        DatabaseError::DatabaseRowNotFound {
          row_id: row_id.clone(),
          reason: "the row is not exist in local disk".to_string(),
        }
      }),
      Some(row) => Ok(row),
    }
  }

  pub async fn init_row_instance(
    &self,
    row_id: RowId,
  ) -> Result<Arc<RwLock<DatabaseRow>>, DatabaseError> {
    trace!("init row instance: {}", row_id);
    let collab = self.create_collab_for_row(&row_id, None).await?;
    let database_row = DatabaseRow::open(
      row_id.clone(),
      collab,
      self.row_change_tx.clone(),
      self.collab_service.clone(),
    )?;
    let row_details = RowDetail::from_collab(&database_row);
    let database_row = Arc::new(RwLock::from(database_row));
    self.row_mem_cache.insert(row_id, database_row.clone());
    if let Some(row_detail) = row_details {
      let _ = self
        .notifier
        .send(BlockEvent::DidFetchRow(vec![row_detail]));
    }
    Ok(database_row)
  }

  async fn create_collab_for_row(
    &self,
    row_id: &RowId,
    encoded_collab: Option<EncodedCollab>,
  ) -> Result<Collab, DatabaseError> {
    self
      .collab_service
      .build_collab(row_id, CollabType::DatabaseRow, encoded_collab)
      .await
  }
}
