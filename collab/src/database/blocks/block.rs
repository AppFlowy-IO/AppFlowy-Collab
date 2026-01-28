use crate::database::rows::{
  Cell, CreateRowParams, DatabaseRow, Row, RowChangeSender, RowDetail, RowMeta, RowMetaKey,
  RowMetaUpdate, RowUpdate, meta_id_from_row_id,
};
use crate::database::views::RowOrder;
use crate::entity::uuid_validation::{DatabaseId, RowId};
use crate::error::CollabError;

use crate::lock::RwLock;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio::sync::broadcast::Sender;

use crate::database::database_trait::{DatabaseRowCollabService, DatabaseRowDataVariant};
use dashmap::DashMap;
use tracing::{debug, instrument, trace};
use yrs::block::ClientID;

#[derive(Clone, Debug)]
pub enum BlockEvent {
  /// The Row is fetched from the remote.
  DidFetchRow(Vec<RowDetail>),
}

pub type InitRowChan = tokio::sync::oneshot::Sender<Result<Arc<RwLock<DatabaseRow>>, CollabError>>;

/// Each [Block] contains a list of [DatabaseRow]s. Each [DatabaseRow] represents a row in the database.
/// Currently, we only use one [Block] to manage all the rows in the database. In the future, we
/// might want to split the rows into multiple [Block]s to improve performance.
#[derive(Clone)]
pub struct Block {
  database_id: DatabaseId,
  collab_service: Arc<dyn DatabaseRowCollabService>,
  pub notifier: Arc<Sender<BlockEvent>>,
  row_change_tx: Option<RowChangeSender>,
  inflight_row_init: Arc<DashMap<RowId, Arc<tokio::sync::Mutex<()>>>>,
}

impl Block {
  pub fn new(
    database_id: DatabaseId,
    collab_service: Arc<dyn DatabaseRowCollabService>,
    row_change_tx: Option<RowChangeSender>,
  ) -> Block {
    let (notifier, _) = broadcast::channel(1000);
    Self {
      database_id,
      collab_service,
      notifier: Arc::new(notifier),
      row_change_tx,
      inflight_row_init: Arc::new(DashMap::new()),
    }
  }

  pub fn subscribe_event(&self) -> broadcast::Receiver<BlockEvent> {
    self.notifier.subscribe()
  }

  pub async fn batch_load_rows(&self, row_ids: Vec<RowId>) -> Result<(), CollabError> {
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
    T: Into<CreateRowParams> + Send,
  {
    let mut row_orders = Vec::with_capacity(rows.len());
    for row in rows {
      if let Ok(row_order) = self.create_new_row(row, client_id).await {
        row_orders.push(row_order);
      }
    }
    row_orders
  }

  pub async fn create_new_row<T: Into<CreateRowParams>>(
    &self,
    row_params: T,
    _client_id: ClientID,
  ) -> Result<RowOrder, CollabError> {
    let params = row_params.into();
    let row: Row = params.clone().into();
    let row_id = row.id;
    let row_order = RowOrder {
      id: row.id,
      height: row.height,
    };

    trace!("creating new database row: {}", row_id);
    let database_row = self
      .collab_service
      .create_arc_database_row(
        &row_id,
        DatabaseRowDataVariant::Row(row),
        self.row_change_tx.clone(),
      )
      .await?;

    if let Some(row_meta) = params.row_meta {
      let mut write_guard = database_row.write().await;
      write_guard.update_meta(|update| {
        update
          .insert_icon_if_not_none(row_meta.icon_url)
          .insert_cover_if_not_none(row_meta.cover)
          .update_is_document_empty_if_not_none(Some(row_meta.is_document_empty))
          .update_attachment_count_if_not_none(Some(row_meta.attachment_count));
      });
    }

    trace!("created new database row: {}", row_id);
    Ok(row_order)
  }

  #[instrument(level = "debug", skip_all)]
  pub fn get_cached_database_row(&self, row_id: &RowId) -> Option<Arc<RwLock<DatabaseRow>>> {
    let cache = self.collab_service.database_row_cache()?;
    cache.get(row_id).map(|row| row.clone())
  }

  /// Return the [DatabaseRow], initializing it on demand if needed.
  /// Use [Self::get_cached_database_row] for cache-only access.
  #[instrument(level = "debug", skip_all)]
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
    Some(meta_id_from_row_id(row_id, RowMetaKey::DocumentId))
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

    let row_ids: Vec<RowId> = row_orders.iter().map(|order| order.id).collect();
    if let Ok(database_rows) = self.init_database_rows(row_ids, auto_fetch).await {
      for database_row in database_rows {
        let read_guard = database_row.read().await;
        let row_id = read_guard.row_id;
        let row = read_guard
          .get_row()
          .unwrap_or_else(|| Row::empty(row_id, self.database_id));
        rows.push(row);
      }
    }

    rows
  }

  #[instrument(level = "debug", skip_all)]
  pub async fn update_row<F>(&mut self, row_id: RowId, f: F)
  where
    F: FnOnce(RowUpdate),
  {
    let result = self.get_or_init_database_row(&row_id).await;
    if let Ok(database_row) = result {
      database_row.write().await.update::<F>(f);
    }
  }

  #[instrument(level = "debug", skip_all)]
  pub async fn update_row_meta<F>(&mut self, row_id: &RowId, f: F)
  where
    F: FnOnce(RowMetaUpdate),
  {
    if let Ok(row) = self.get_or_init_database_row(row_id).await {
      row.write().await.update_meta::<F>(f);
    }
  }

  /// Initialize the [DatabaseRow] in the background and optionally return it via channel.
  /// Also sends a [BlockEvent::DidFetchRow] notification so Flutter can update row metadata.
  #[instrument(level = "debug", skip_all)]
  pub fn init_database_row(&self, row_id: &RowId, ret: Option<InitRowChan>) {
    let block = self.clone();
    let row_id = *row_id;
    let notifier = self.notifier.clone();
    tokio::task::spawn(async move {
      let row = block.get_or_init_database_row(&row_id).await;

      // Send DidFetchRow notification on successful load so Flutter can update
      // row metadata (e.g., created_by) that isn't included in lightweight RowOrder.
      if let Ok(ref row_lock) = row {
        let guard = row_lock.read().await;
        if let Some(row_detail) = RowDetail::from_collab(&guard) {
          debug!(
            "[init_database_row] row_id={}, created_by={:?}",
            row_id, row_detail.row.created_by
          );
          drop(guard);
          let _ = notifier.send(BlockEvent::DidFetchRow(vec![row_detail]));
        } else {
          debug!(
            "[init_database_row] row_id={}, RowDetail::from_collab returned None",
            row_id
          );
        }
      } else {
        debug!(
          "[init_database_row] row_id={}, get_or_init_database_row failed",
          row_id
        );
      }

      if let Some(ret) = ret {
        let _ = ret.send(row);
      }
    });
  }

  #[instrument(level = "debug", skip_all)]
  pub async fn get_or_init_database_row(
    &self,
    row_id: &RowId,
  ) -> Result<Arc<RwLock<DatabaseRow>>, CollabError> {
    if let Some(row) = self.get_cached_database_row(row_id) {
      return Ok(row);
    }

    let init_lock = {
      // Drop DashMap guard before awaiting the per-row mutex.
      let entry = self
        .inflight_row_init
        .entry(*row_id)
        .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())));
      entry.clone()
    };

    let _guard = init_lock.lock().await;
    if let Some(row) = self.get_cached_database_row(row_id) {
      drop(_guard);
      self.inflight_row_init.remove(row_id);
      return Ok(row);
    }

    let result = self
      .collab_service
      .build_arc_database_row(row_id, None, self.row_change_tx.clone())
      .await;

    drop(_guard);
    self.inflight_row_init.remove(row_id);

    result
  }

  pub async fn init_database_rows(
    &self,
    row_ids: Vec<RowId>,
    auto_fetch: bool,
  ) -> Result<Vec<Arc<RwLock<DatabaseRow>>>, CollabError> {
    // Retain only rows that are not in the cache
    let uncached_rows = self
      .collab_service
      .batch_build_arc_database_row(&row_ids, self.row_change_tx.clone(), auto_fetch)
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
