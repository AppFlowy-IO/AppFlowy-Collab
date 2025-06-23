use crate::error::DatabaseError;
use crate::rows::{
  Cell, DatabaseRow, Row, RowChangeSender, RowDetail, RowId, RowMeta, RowMetaKey, RowMetaUpdate,
  RowUpdate, meta_id_from_row_id,
};
use crate::views::RowOrder;

use collab::lock::RwLock;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio::sync::broadcast::Sender;

use crate::database_trait::{DatabaseRowCollabService, DatabaseRowDataVariant};
use tracing::{instrument, trace};
use uuid::Uuid;
use yrs::block::ClientID;

#[derive(Clone, Debug)]
pub enum BlockEvent {
  /// The Row is fetched from the remote.
  DidFetchRow(Vec<RowDetail>),
}

pub type InitRowChan =
  tokio::sync::oneshot::Sender<Result<Arc<RwLock<DatabaseRow>>, DatabaseError>>;

/// Each [Block] contains a list of [DatabaseRow]s. Each [DatabaseRow] represents a row in the database.
/// Currently, we only use one [Block] to manage all the rows in the database. In the future, we
/// might want to split the rows into multiple [Block]s to improve performance.
#[derive(Clone)]
pub struct Block {
  database_id: String,
  collab_service: Arc<dyn DatabaseRowCollabService>,
  pub notifier: Arc<Sender<BlockEvent>>,
  row_change_tx: Option<RowChangeSender>,
}

impl Block {
  pub fn new(
    database_id: String,
    collab_service: Arc<dyn DatabaseRowCollabService>,
    row_change_tx: Option<RowChangeSender>,
  ) -> Block {
    let (notifier, _) = broadcast::channel(1000);
    Self {
      database_id,
      collab_service,
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
      let row = self
        .collab_service
        .build_arc_database_row(&row_id, None, self.row_change_tx.clone())
        .await?;

      let guard = row.read().await;
      if let Some(row_detail) = RowDetail::from_collab(&guard) {
        row_on_disk_details.push(row_detail);
      }
      drop(guard);
    }

    if !row_on_disk_details.is_empty() {
      let _ = cloned_notifier.send(BlockEvent::DidFetchRow(row_on_disk_details));
    }
    Ok(())
  }

  pub async fn create_rows<T>(&self, rows: Vec<T>, client_id: ClientID) -> Vec<RowOrder>
  where
    T: Into<Row> + Send,
  {
    let mut row_orders = Vec::with_capacity(rows.len());
    for row in rows {
      if let Ok(row_order) = self.create_new_row(row, client_id).await {
        row_orders.push(row_order);
      }
    }
    row_orders
  }

  pub async fn create_new_row<T: Into<Row>>(
    &self,
    row: T,
    _client_id: ClientID,
  ) -> Result<RowOrder, DatabaseError> {
    let row = row.into();
    let row_id = row.id.clone();
    let row_order = RowOrder {
      id: row.id.clone(),
      height: row.height,
    };

    trace!("creating new database row: {}", row_id);
    let _ = self
      .collab_service
      .create_arc_database_row(
        &row_id,
        DatabaseRowDataVariant::Row(row),
        self.row_change_tx.clone(),
      )
      .await?;

    trace!("created new database row: {}", row_id);
    Ok(row_order)
  }

  pub async fn get_database_row(&self, row_id: &RowId) -> Option<Arc<RwLock<DatabaseRow>>> {
    self.get_or_init_database_row(row_id).await.ok()
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
  pub async fn get_rows_from_row_orders(
    &self,
    row_orders: &[RowOrder],
    auto_fetch: bool,
  ) -> Vec<Row> {
    let mut rows = Vec::new();

    let row_ids: Vec<RowId> = row_orders.iter().map(|order| order.id.clone()).collect();
    if let Ok(database_rows) = self.init_database_rows(row_ids, auto_fetch).await {
      for database_row in database_rows {
        let read_guard = database_row.read().await;
        let row_id = read_guard.row_id.clone();
        let row = read_guard
          .get_row()
          .unwrap_or_else(|| Row::empty(row_id, &self.database_id));
        rows.push(row);
      }
    }

    rows
  }

  pub async fn update_row<F>(&mut self, row_id: RowId, f: F)
  where
    F: FnOnce(RowUpdate),
  {
    let result = self.get_or_init_database_row(&row_id).await;
    if let Ok(database_row) = result {
      database_row.write().await.update::<F>(f);
    }
  }

  pub async fn update_row_meta<F>(&mut self, row_id: &RowId, f: F)
  where
    F: FnOnce(RowMetaUpdate),
  {
    if let Ok(row) = self.get_or_init_database_row(row_id).await {
      row.write().await.update_meta::<F>(f);
    }
  }

  /// Get the [DatabaseRow] from the cache. If the row is not in the cache, initialize it.
  pub fn init_database_row(&self, row_id: &RowId, ret: Option<InitRowChan>) {
    let row_id = row_id.clone();
    let row_change_tx = self.row_change_tx.clone();
    let collab_service = self.collab_service.clone();
    tokio::task::spawn(async move {
      let row = collab_service
        .build_arc_database_row(&row_id, None, row_change_tx)
        .await;

      if let Some(ret) = ret {
        let _ = ret.send(row);
      }
    });
  }

  pub async fn get_or_init_database_row(
    &self,
    row_id: &RowId,
  ) -> Result<Arc<RwLock<DatabaseRow>>, DatabaseError> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    self.init_database_row(row_id, Some(tx));
    rx.await
      .map_err(|e| DatabaseError::Internal(anyhow::anyhow!(e)))?
  }

  pub async fn init_database_rows(
    &self,
    row_ids: Vec<RowId>,
    auto_fetch: bool,
  ) -> Result<Vec<Arc<RwLock<DatabaseRow>>>, DatabaseError> {
    // Retain only rows that are not in the cache
    let uncached_row_ids: Vec<String> = row_ids.iter().map(|id| id.to_string()).collect();
    let uncached_rows = self
      .collab_service
      .batch_build_arc_database_row(&uncached_row_ids, self.row_change_tx.clone(), auto_fetch)
      .await?;

    // Initialize final database rows by combining cached and newly fetched rows
    let mut database_rows = Vec::with_capacity(row_ids.len());
    for row_id in row_ids {
      if let Some(new_row) = uncached_rows.get(&row_id) {
        database_rows.push(new_row.clone());
      }
    }

    Ok(database_rows)
  }
}
