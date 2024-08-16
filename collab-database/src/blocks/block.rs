use dashmap::DashMap;

use dashmap::mapref::one::RefMut;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Weak};

use collab::error::CollabError;
use collab_entity::CollabType;
use collab_plugins::local_storage::kv::doc::CollabKVAction;
use collab_plugins::local_storage::kv::KVTransactionDB;
use collab_plugins::local_storage::CollabPersistenceConfig;
use collab_plugins::CollabKVDB;

use crate::blocks::task_controller::{BlockTask, BlockTaskController};
use crate::error::DatabaseError;
use crate::rows::{
  meta_id_from_row_id, Cell, DatabaseRow, Row, RowChangeSender, RowDetail, RowId, RowMeta,
  RowMetaKey, RowMetaUpdate, RowUpdate,
};
use crate::views::RowOrder;
use crate::workspace_database::DatabaseCollabService;
use collab::preclude::Collab;
use collab_plugins::local_storage::rocksdb::util::KVDBCollabPersistenceImpl;
use tokio::sync::broadcast;
use tokio::sync::broadcast::Sender;
use tracing::{error, trace, warn};
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
  uid: i64,
  database_id: String,
  collab_db: Weak<CollabKVDB>,
  collab_service: Arc<dyn DatabaseCollabService>,
  task_controller: Arc<BlockTaskController>,
  sequence: Arc<AtomicU32>,
  pub rows: Arc<DashMap<RowId, DatabaseRow>>,
  pub notifier: Arc<Sender<BlockEvent>>,
  row_change_tx: RowChangeSender,
}

impl Block {
  pub fn new(
    uid: i64,
    database_id: String,
    collab_db: Weak<CollabKVDB>,
    collab_service: Arc<dyn DatabaseCollabService>,
    row_change_tx: RowChangeSender,
  ) -> Block {
    let controller = BlockTaskController::new(collab_db.clone(), Arc::downgrade(&collab_service));
    let task_controller = Arc::new(controller);
    let (notifier, _) = broadcast::channel(1000);
    Self {
      uid,
      database_id,
      collab_db,
      task_controller,
      collab_service,
      sequence: Arc::new(Default::default()),
      rows: Arc::new(Default::default()),
      notifier: Arc::new(notifier),
      row_change_tx,
    }
  }

  pub fn subscribe_event(&self) -> broadcast::Receiver<BlockEvent> {
    self.notifier.subscribe()
  }

  pub async fn batch_load_rows(&self, row_ids: Vec<RowId>) {
    let (tx, mut rx) = tokio::sync::mpsc::channel(1);
    self.task_controller.add_task(BlockTask::BatchFetchRow {
      uid: self.uid,
      row_ids,
      seq: self.sequence.fetch_add(1, Ordering::SeqCst),
      sender: tx,
    });

    let uid = self.uid;
    let change_tx = self.row_change_tx.clone();
    let collab_db = self.collab_db.clone();
    let cache = self.rows.clone();
    let weak_notifier = Arc::downgrade(&self.notifier);
    while let Some(row_collabs) = rx.recv().await {
      for (row_id, row_collab) in row_collabs {
        match row_collab {
          Ok(row_collab) => {
            if let Err(err) = Self::create_collab_row_instance(
              &RowId::from(row_id),
              weak_notifier.clone(),
              uid,
              change_tx.clone(),
              collab_db.clone(),
              cache.clone(),
              row_collab,
            ) {
              error!("Can't init collab row: {:?}", err);
            }
          },
          Err(err) => {
            error!("Can't fetch the row from remote: {:?}", err);
          },
        }
      }
    }
  }

  pub fn create_rows<T>(&self, rows: Vec<T>) -> Vec<RowOrder>
  where
    T: Into<Row> + Send,
  {
    let mut row_orders = Vec::with_capacity(rows.len());
    for row in rows {
      let row_order = self.create_row(row);
      row_orders.push(row_order);
    }
    row_orders
  }

  pub fn create_row<T: Into<Row>>(&self, row: T) -> RowOrder {
    let row = row.into();
    let row_id = row.id.clone();
    let row_order = RowOrder {
      id: row.id.clone(),
      height: row.height,
    };

    trace!("create_row: {}", row_id);
    if let Ok(collab) = self.create_collab_for_row(&row_id) {
      let database_row = DatabaseRow::new(
        self.uid,
        row_id.clone(),
        self.collab_db.clone(),
        collab,
        self.row_change_tx.clone(),
        Some(row),
      );
      self.rows.insert(row_id, database_row);
    }
    row_order
  }

  pub fn get_row_meta(&self, row_id: &RowId) -> Option<RowMeta> {
    let database_row = self.rows.get(row_id)?;
    database_row.get_row_meta()
  }

  pub fn get_row_document_id(&self, row_id: &RowId) -> Option<String> {
    let row_id = Uuid::parse_str(row_id).ok()?;
    Some(meta_id_from_row_id(&row_id, RowMetaKey::DocumentId))
  }

  /// If the row with given id not exist. It will return an empty row with given id.
  /// An empty [Row] is a row with no cells.
  ///
  pub fn get_rows_from_row_orders(&self, row_orders: &[RowOrder]) -> Vec<Row> {
    let mut rows = Vec::new();
    for row_order in row_orders {
      let row = self
        .get_or_init_row(row_order.id.clone())
        .and_then(|row| row.get_row())
        .unwrap_or_else(|| Row::empty(row_order.id.clone(), &self.database_id));
      rows.push(row);
    }
    rows
  }

  pub fn get_cell(&self, row_id: &RowId, field_id: &str) -> Option<Cell> {
    self
      .get_or_init_row(row_id.clone())
      .and_then(|row| row.get_cell(field_id))
  }

  pub fn delete_row(&self, row_id: &RowId) -> Option<DatabaseRow> {
    let row = self.rows.remove(row_id);
    if let Some((_, row)) = row {
      row.delete();
      Some(row)
    } else {
      None
    }
  }

  pub fn update_row<F>(&mut self, row_id: RowId, f: F)
  where
    F: FnOnce(RowUpdate),
  {
    if let Some(mut row) = self.get_or_init_row(row_id) {
      row.update::<F>(f);
    }
  }

  pub fn update_row_meta<F>(&mut self, row_id: &RowId, f: F)
  where
    F: FnOnce(RowMetaUpdate),
  {
    let row = self.rows.get_mut(row_id);
    match row {
      None => {
        trace!(
          "fail to update row meta. the row is not in the cache: {:?}",
          row_id
        )
      },
      Some(mut row) => {
        row.value_mut().update_meta::<F>(f);
      },
    }
  }

  /// Get the [DatabaseRow] from the cache. If the row is not in the cache, initialize it.
  pub fn get_or_init_row(&self, row_id: RowId) -> Option<RefMut<RowId, DatabaseRow>> {
    let result = self
      .rows
      .entry(row_id.clone())
      .or_try_insert_with(|| self.create_row_instance(row_id));

    match result {
      Ok(row) => Some(row),
      Err(err) => {
        warn!("failed to initialize row: {err}");
        None
      },
    }
  }

  fn create_row_instance(&self, row_id: RowId) -> Result<DatabaseRow, DatabaseError> {
    let collab_db = self
      .collab_db
      .upgrade()
      .ok_or(DatabaseError::DatabaseNotExist)?;
    let exists = collab_db.read_txn().is_exist(self.uid, row_id.as_ref());
    // Can't find the row in local disk, fetch it from remote.
    if !exists {
      trace!(
        "Can't find the row in local disk, fetch it from remote: {:?}",
        row_id
      );
      let (sender, mut rx) = tokio::sync::mpsc::channel(1);
      self.task_controller.add_task(BlockTask::FetchRow {
        uid: self.uid,
        row_id: row_id.clone(),
        seq: self.sequence.fetch_add(1, Ordering::SeqCst),
        sender,
      });

      let weak_notifier = Arc::downgrade(&self.notifier);
      let uid = self.uid;
      let change_tx = self.row_change_tx.clone();
      let weak_collab_db = self.collab_db.clone();
      let cache = self.rows.clone();
      let rid = row_id.clone();
      tokio::spawn(async move {
        if let Some(Ok(row_collab)) = rx.recv().await {
          if let Err(err) = Self::create_collab_row_instance(
            &rid,
            weak_notifier,
            uid,
            change_tx,
            weak_collab_db.clone(),
            cache,
            row_collab,
          ) {
            error!("Can't init collab row: {:?}", err);
            if let Some(collab_db) = weak_collab_db.upgrade() {
              let _ = collab_db.with_write_txn(|txn| {
                txn.delete_doc(uid, rid.as_ref())?;
                Ok(())
              });
            }
          }
        } else {
          error!("Can't fetch the row from remote: {:?}", rid);
        }
      });
      Err(DatabaseError::DatabaseRowNotExist(row_id))
    } else {
      let collab = self.create_collab_for_row(&row_id)?;
      let database_row = DatabaseRow::new(
        self.uid,
        row_id.clone(),
        self.collab_db.clone(),
        collab,
        self.row_change_tx.clone(),
        None,
      );
      Ok(database_row)
    }
  }

  fn create_collab_row_instance(
    row_id: &RowId,
    weak_notifier: Weak<Sender<BlockEvent>>,
    uid: i64,
    change_tx: RowChangeSender,
    collab_db: Weak<CollabKVDB>,
    cache: Arc<DashMap<RowId, DatabaseRow>>,
    row_collab: Collab,
  ) -> Result<(), CollabError> {
    if cache.contains_key(row_id) {
      warn!("The row is already in the cache: {:?}", row_id);
      return Ok(());
    }

    trace!("init_collab_row: {:?}", row_id);
    let row_detail = RowDetail::from_collab(&row_collab);
    let row = DatabaseRow::new(uid, row_id.clone(), collab_db, row_collab, change_tx, None);
    cache.insert(row_id.clone(), row);

    if let Some(notifier) = weak_notifier.upgrade() {
      match row_detail {
        None => {
          error!("Can't get the row detail information from collab");
        },
        Some(row_detail) => {
          let _ = notifier.send(BlockEvent::DidFetchRow(vec![row_detail]));
        },
      }
    }
    Ok(())
  }

  fn create_collab_for_row(&self, row_id: &RowId) -> Result<Collab, DatabaseError> {
    let config = CollabPersistenceConfig::new().snapshot_per_update(100);
    let data_source = KVDBCollabPersistenceImpl {
      db: self.collab_db.clone(),
      uid: self.uid,
    };
    self.collab_service.build_collab_with_config(
      self.uid,
      row_id,
      CollabType::DatabaseRow,
      self.collab_db.clone(),
      data_source.into(),
      config,
    )
  }
}
