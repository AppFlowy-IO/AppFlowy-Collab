use std::ops::Deref;
use std::sync::{Arc, Weak};

use collab::core::collab::MutexCollab;
use collab::preclude::{
  Any, ArrayRefWrapper, Collab, DeepEventsSubscription, Map, MapPrelim, MapRef, MapRefExtension,
  MapRefWrapper, ReadTxn, Transaction, TransactionMut, YrsValue,
};
use parking_lot::Mutex;

use collab::core::value::YrsValueExtension;
use collab_plugins::local_storage::kv::doc::CollabKVAction;
use collab_plugins::local_storage::kv::KVTransactionDB;
use collab_plugins::CollabKVDB;
use serde::{Deserialize, Serialize};
use tracing::error;
use uuid::Uuid;

use crate::database::{gen_row_id, timestamp};
use crate::error::DatabaseError;
use crate::rows::{
  subscribe_row_data_change, Cell, Cells, CellsUpdate, RowChangeSender, RowId, RowMeta,
  RowMetaUpdate,
};
use crate::views::{OrderObjectPosition, RowOrder};
use crate::{impl_bool_update, impl_i32_update, impl_i64_update};

pub type BlockId = i64;

const DATA: &str = "data";
const META: &str = "meta";
const COMMENT: &str = "comment";
pub const LAST_MODIFIED: &str = "last_modified";
pub const CREATED_AT: &str = "created_at";

pub struct DatabaseRow {
  uid: i64,
  row_id: RowId,
  #[allow(dead_code)]
  collab: Arc<MutexCollab>,
  data: MapRefWrapper,
  meta: MapRefWrapper,
  #[allow(dead_code)]
  comments: ArrayRefWrapper,
  collab_db: Weak<CollabKVDB>,
  #[allow(dead_code)]
  subscription: Option<DeepEventsSubscription>,
}

impl DatabaseRow {
  pub fn create<T: Into<Row>>(
    row: T,
    uid: i64,
    row_id: RowId,
    collab_db: Weak<CollabKVDB>,
    collab: Arc<MutexCollab>,
    change_tx: Option<RowChangeSender>,
  ) -> Self {
    let row = row.into();
    let mut database_row = Self::inner_new(uid, row_id, collab_db, collab);
    let data = database_row.data.clone();
    let meta = database_row.meta.clone();
    database_row.collab.lock().with_origin_transact_mut(|txn| {
      RowBuilder::new(txn, data.into_inner(), meta.into_inner())
        .update(|update| {
          update
            .set_row_id(row.id)
            .set_height(row.height)
            .set_visibility(row.visibility)
            .set_created_at(row.created_at)
            .set_last_modified(row.created_at)
            .set_cells(row.cells);
        })
        .done();
    });

    database_row.subscription =
      change_tx.map(|sender| subscribe_row_data_change(&mut database_row.data, sender));

    database_row
  }

  pub fn new(
    uid: i64,
    row_id: RowId,
    collab_db: Weak<CollabKVDB>,
    collab: Arc<MutexCollab>,
    change_tx: Option<RowChangeSender>,
  ) -> Self {
    let mut this = Self::inner_new(uid, row_id, collab_db, collab);
    this.subscription = change_tx.map(|sender| subscribe_row_data_change(&mut this.data, sender));
    this
  }

  fn inner_new(
    uid: i64,
    row_id: RowId,
    collab_db: Weak<CollabKVDB>,
    collab: Arc<MutexCollab>,
  ) -> Self {
    let collab_guard = collab.lock();
    let (data, meta, comments) = {
      let txn = collab_guard.transact();
      let data = collab_guard.get_map_with_txn(&txn, vec![DATA]);
      let meta = collab_guard.get_map_with_txn(&txn, vec![META]);
      let comments = collab_guard.get_array_with_txn(&txn, vec![COMMENT]);
      drop(txn);
      (data, meta, comments)
    };

    // If any of the data is missing, we need to create it.
    let mut txn = if data.is_none() || meta.is_none() || comments.is_none() {
      Some(collab_guard.origin_transact_mut())
    } else {
      None
    };

    let data =
      data.unwrap_or_else(|| collab_guard.insert_map_with_txn(txn.as_mut().unwrap(), DATA));
    let meta =
      meta.unwrap_or_else(|| collab_guard.insert_map_with_txn(txn.as_mut().unwrap(), META));
    let comments = comments.unwrap_or_else(|| {
      collab_guard.create_array_with_txn::<MapPrelim<Any>>(txn.as_mut().unwrap(), COMMENT, vec![])
    });

    drop(txn);
    drop(collab_guard);

    Self {
      uid,
      row_id,
      collab,
      data,
      meta,
      comments,
      collab_db,
      subscription: None,
    }
  }

  pub fn get_row(&self) -> Option<Row> {
    let collab = self.collab.try_lock()?;
    let txn = collab.try_transaction().ok()?;
    row_from_map_ref(&self.data, &self.meta, &txn)
  }

  pub fn get_row_meta(&self) -> Option<RowMeta> {
    let collab = self.collab.try_lock()?;
    let txn = collab.try_transaction().ok()?;
    let row_id = Uuid::parse_str(&self.row_id).ok()?;
    Some(RowMeta::from_map_ref(&txn, &row_id, &self.meta))
  }

  pub fn get_row_order(&self) -> Option<RowOrder> {
    let collab = self.collab.try_lock()?;
    let txn = collab.try_transaction().ok()?;
    row_order_from_map_ref(&self.data, &txn).map(|value| value.0)
  }

  pub fn get_cell(&self, field_id: &str) -> Option<Cell> {
    let collab = self.collab.try_lock()?;
    let txn = collab.try_transaction().ok()?;
    cell_from_map_ref(&self.data, &txn, field_id)
  }

  pub fn update<F>(&self, f: F)
  where
    F: FnOnce(RowUpdate),
  {
    match self.collab.try_lock() {
      None => error!("failed to acquire lock for updating row"),
      Some(guard) => {
        guard.with_origin_transact_mut(|txn| {
          let mut update = RowUpdate::new(txn, &self.data, &self.meta);

          // Update the last modified timestamp before we call the update function.
          update = update.set_last_modified(timestamp());
          f(update)
        });
      },
    }
  }

  pub fn update_meta<F>(&self, f: F)
  where
    F: FnOnce(RowMetaUpdate),
  {
    self
      .collab
      .lock()
      .with_origin_transact_mut(|txn| match Uuid::parse_str(&self.row_id) {
        Ok(row_id) => {
          let update = RowMetaUpdate::new(txn, &self.meta, row_id);
          f(update)
        },
        Err(e) => error!("🔴 can't update the row meta: {}", e),
      })
  }

  pub fn delete(&self) {
    match self.collab_db.upgrade() {
      None => {
        tracing::warn!("collab db is drop when delete a collab object");
      },
      Some(collab_db) => {
        let _ = collab_db.with_write_txn(|txn| {
          let row_id = self.row_id.to_string();
          if let Err(e) = txn.delete_doc(self.uid, &row_id) {
            error!("🔴{}", e);
          }
          Ok(())
        });
      },
    }
  }
}

#[derive(Debug, Clone)]
pub struct RowDetail {
  pub row: Row,
  pub meta: RowMeta,
  pub document_id: String,
}

impl RowDetail {
  pub fn new(row: Row, meta: RowMeta) -> Option<Self> {
    let row_id = Uuid::parse_str(&row.id).ok()?;
    let document_id = meta_id_from_row_id(&row_id, RowMetaKey::DocumentId);
    Some(Self {
      row,
      meta,
      document_id,
    })
  }
  pub fn from_collab(collab: &Collab, txn: &Transaction) -> Option<Self> {
    let data = collab.get_map_with_txn(txn, vec![DATA])?;
    let meta = collab.get_map_with_txn(txn, vec![META])?;
    let row = row_from_map_ref(&data, &meta, txn)?;

    let row_id = Uuid::parse_str(&row.id).ok()?;
    let meta = RowMeta::from_map_ref(txn, &row_id, &meta);
    let row_document_id = meta_id_from_row_id(&row_id, RowMetaKey::DocumentId);
    Some(Self {
      row,
      meta,
      document_id: row_document_id,
    })
  }
}

/// Represents a row in a [Block].
/// A [Row] contains list of [Cell]s. Each [Cell] is associated with a [Field].
/// So the number of [Cell]s in a [Row] is equal to the number of [Field]s.
/// A [Database] contains list of rows that stored in multiple [Block]s.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Row {
  pub id: RowId,
  pub cells: Cells,
  pub height: i32,
  pub visibility: bool,
  pub created_at: i64,
  pub modified_at: i64,
}

pub enum RowMetaKey {
  DocumentId,
  IconId,
  CoverId,
  IsDocumentEmpty,
}

impl RowMetaKey {
  pub fn as_str(&self) -> &str {
    match self {
      Self::DocumentId => "document_id",
      Self::IconId => "icon_id",
      Self::CoverId => "cover_id",
      Self::IsDocumentEmpty => "is_document_empty",
    }
  }
}

const DEFAULT_ROW_HEIGHT: i32 = 60;
impl Row {
  /// Creates a new instance of [Row]
  /// The default height of a [Row] is 60
  /// The default visibility of a [Row] is true
  /// The default created_at of a [Row] is the current timestamp
  pub fn new<R: Into<RowId>>(id: R) -> Self {
    let timestamp = timestamp();
    Row {
      id: id.into(),
      cells: Default::default(),
      height: DEFAULT_ROW_HEIGHT,
      visibility: true,
      created_at: timestamp,
      modified_at: timestamp,
    }
  }

  pub fn empty(row_id: RowId) -> Self {
    Self {
      id: row_id,
      cells: Cells::new(),
      height: DEFAULT_ROW_HEIGHT,
      visibility: true,
      created_at: 0,
      modified_at: 0,
    }
  }

  pub fn is_empty(&self) -> bool {
    self.cells.is_empty()
  }

  pub fn document_id(&self) -> String {
    meta_id_from_meta_type(self.id.as_str(), RowMetaKey::DocumentId)
  }

  pub fn icon_id(&self) -> String {
    meta_id_from_meta_type(self.id.as_str(), RowMetaKey::IconId)
  }

  pub fn cover_id(&self) -> String {
    meta_id_from_meta_type(self.id.as_str(), RowMetaKey::CoverId)
  }
}

pub fn database_row_document_id_from_row_id(row_id: &str) -> String {
  meta_id_from_meta_type(row_id, RowMetaKey::DocumentId)
}

fn meta_id_from_meta_type(row_id: &str, key: RowMetaKey) -> String {
  match Uuid::parse_str(row_id) {
    Ok(row_id_uuid) => meta_id_from_row_id(&row_id_uuid, key),
    Err(e) => {
      // This should never happen. Because the row_id generated by gen_row_id() is always
      // a valid uuid.
      error!("🔴Invalid row_id: {}, error:{:?}", row_id, e);
      Uuid::new_v4().to_string()
    },
  }
}

pub fn meta_id_from_row_id(row_id: &Uuid, key: RowMetaKey) -> String {
  Uuid::new_v5(row_id, key.as_str().as_bytes()).to_string()
}

pub struct RowBuilder<'a, 'b> {
  map_ref: MapRef,
  meta_ref: MapRef,
  txn: &'a mut TransactionMut<'b>,
}

impl<'a, 'b> RowBuilder<'a, 'b> {
  pub fn new(txn: &'a mut TransactionMut<'b>, map_ref: MapRef, meta_ref: MapRef) -> Self {
    Self {
      map_ref,
      meta_ref,
      txn,
    }
  }

  pub fn update<F>(self, f: F) -> Self
  where
    F: FnOnce(RowUpdate),
  {
    let update = RowUpdate::new(self.txn, &self.map_ref, &self.meta_ref);
    f(update);
    self
  }
  pub fn done(self) {}
}

/// It used to update a [Row]
pub struct RowUpdate<'a, 'b, 'c> {
  map_ref: &'c MapRef,
  meta_ref: &'c MapRef,
  txn: &'a mut TransactionMut<'b>,
}

impl<'a, 'b, 'c> RowUpdate<'a, 'b, 'c> {
  pub fn new(txn: &'a mut TransactionMut<'b>, map_ref: &'c MapRef, meta_ref: &'c MapRef) -> Self {
    Self {
      map_ref,
      txn,
      meta_ref,
    }
  }

  impl_bool_update!(set_visibility, set_visibility_if_not_none, ROW_VISIBILITY);
  impl_i32_update!(set_height, set_height_at_if_not_none, ROW_HEIGHT);
  impl_i64_update!(set_created_at, set_created_at_if_not_none, CREATED_AT);
  impl_i64_update!(
    set_last_modified,
    set_last_modified_if_not_none,
    LAST_MODIFIED
  );

  pub fn set_row_id(self, new_row_id: RowId) -> Self {
    let old_row_meta = row_id_from_map_ref(self.txn, self.map_ref)
      .and_then(|row_id| row_id.parse::<Uuid>().ok())
      .map(|row_id| RowMeta::from_map_ref(self.txn, &row_id, self.meta_ref));

    self
      .map_ref
      .insert_str_with_txn(self.txn, ROW_ID, new_row_id.clone());

    if let Ok(new_row_id) = new_row_id.parse::<Uuid>() {
      self.meta_ref.clear(self.txn);
      let mut new_row_meta = RowMeta::empty();
      if let Some(old_row_meta) = old_row_meta {
        new_row_meta.icon_url = old_row_meta.icon_url;
        new_row_meta.cover_url = old_row_meta.cover_url;
      }
      new_row_meta.fill_map_ref(self.txn, &new_row_id, self.meta_ref);
    }

    self
  }

  pub fn set_cells(self, cells: Cells) -> Self {
    let cell_map = self.map_ref.get_or_create_map_with_txn(self.txn, ROW_CELLS);
    cells.fill_map_ref(self.txn, &cell_map);
    self
  }

  pub fn update_cells<F>(self, f: F) -> Self
  where
    F: FnOnce(CellsUpdate),
  {
    let cell_map = self.map_ref.get_or_create_map_with_txn(self.txn, ROW_CELLS);
    let update = CellsUpdate::new(self.txn, &cell_map);
    f(update);
    self
  }

  pub fn done(self) -> Option<Row> {
    row_from_map_ref(self.map_ref, self.meta_ref, self.txn)
  }
}

pub(crate) const ROW_ID: &str = "id";
pub(crate) const ROW_VISIBILITY: &str = "visibility";

pub const ROW_HEIGHT: &str = "height";
pub const ROW_CELLS: &str = "cells";

/// Return row id and created_at from a [YrsValue]
pub fn row_id_from_value<T: ReadTxn>(value: YrsValue, txn: &T) -> Option<(String, i64)> {
  let map_ref = value.to_ymap()?;
  let id = map_ref.get_str_with_txn(txn, ROW_ID)?;
  let crated_at = map_ref
    .get_i64_with_txn(txn, CREATED_AT)
    .unwrap_or_default();
  Some((id, crated_at))
}

/// Return a [RowOrder] and created_at from a [YrsValue]
pub fn row_order_from_value<T: ReadTxn>(value: YrsValue, txn: &T) -> Option<(RowOrder, i64)> {
  let map_ref = value.to_ymap()?;
  row_order_from_map_ref(map_ref, txn)
}

/// Return a [RowOrder] and created_at from a [YrsValue]
pub fn row_order_from_map_ref<T: ReadTxn>(map_ref: &MapRef, txn: &T) -> Option<(RowOrder, i64)> {
  let id = RowId::from(map_ref.get_str_with_txn(txn, ROW_ID)?);
  let height = map_ref.get_i64_with_txn(txn, ROW_HEIGHT).unwrap_or(60);
  let crated_at = map_ref
    .get_i64_with_txn(txn, CREATED_AT)
    .unwrap_or_default();
  Some((RowOrder::new(id, height as i32), crated_at))
}

/// Return a [Cell] in a [Row] from a [YrsValue]
/// The [Cell] is identified by the field_id
pub fn cell_from_map_ref<T: ReadTxn>(map_ref: &MapRef, txn: &T, field_id: &str) -> Option<Cell> {
  let cells_map_ref = map_ref.get_map_with_txn(txn, ROW_CELLS)?;
  let cell_map_ref = cells_map_ref.get_map_with_txn(txn, field_id)?;
  Some(Cell::from_map_ref(txn, &cell_map_ref))
}

pub fn row_id_from_map_ref<T: ReadTxn>(txn: &T, map_ref: &MapRef) -> Option<RowId> {
  map_ref.get_str_with_txn(txn, ROW_ID).map(RowId::from)
}

/// Return a [Row] from a [MapRef]
pub fn row_from_map_ref<T: ReadTxn>(map_ref: &MapRef, _meta_ref: &MapRef, txn: &T) -> Option<Row> {
  let id = RowId::from(map_ref.get_str_with_txn(txn, ROW_ID)?);
  let visibility = map_ref
    .get_bool_with_txn(txn, ROW_VISIBILITY)
    .unwrap_or(true);

  let height = map_ref.get_i64_with_txn(txn, ROW_HEIGHT).unwrap_or(60);

  let created_at = map_ref
    .get_i64_with_txn(txn, CREATED_AT)
    .unwrap_or_else(|| chrono::Utc::now().timestamp());

  let modified_at = map_ref
    .get_i64_with_txn(txn, LAST_MODIFIED)
    .unwrap_or_else(|| chrono::Utc::now().timestamp());

  let cells = map_ref
    .get_map_with_txn(txn, ROW_CELLS)
    .map(|map_ref| (txn, &map_ref).into())
    .unwrap_or_default();

  Some(Row {
    id,
    cells,
    height: height as i32,
    visibility,
    created_at,
    modified_at,
  })
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateRowParams {
  pub id: RowId,
  pub cells: Cells,
  pub height: i32,
  pub visibility: bool,
  #[serde(skip)]
  pub row_position: OrderObjectPosition,
  pub timestamp: i64,
}

pub(crate) struct CreateRowParamsValidator;

impl CreateRowParamsValidator {
  pub(crate) fn validate(mut params: CreateRowParams) -> Result<CreateRowParams, DatabaseError> {
    if params.id.is_empty() {
      return Err(DatabaseError::InvalidRowID("row_id is empty"));
    }

    if params.timestamp == 0 {
      params.timestamp = timestamp();
    }

    Ok(params)
  }
}

impl Default for CreateRowParams {
  fn default() -> Self {
    Self {
      id: gen_row_id(),
      cells: Default::default(),
      height: 60,
      visibility: true,
      row_position: OrderObjectPosition::default(),
      timestamp: 0,
    }
  }
}

impl CreateRowParams {
  pub fn new(id: RowId) -> Self {
    Self {
      id,
      cells: Cells::default(),
      height: 60,
      visibility: true,
      row_position: OrderObjectPosition::default(),
      timestamp: timestamp(),
    }
  }
}

impl From<CreateRowParams> for Row {
  fn from(params: CreateRowParams) -> Self {
    Row {
      id: params.id,
      cells: params.cells,
      height: params.height,
      visibility: params.visibility,
      created_at: params.timestamp,
      modified_at: params.timestamp,
    }
  }
}

#[derive(Clone)]
pub struct MutexDatabaseRow(Arc<Mutex<DatabaseRow>>);

impl MutexDatabaseRow {
  pub fn new(inner: DatabaseRow) -> Self {
    #[allow(clippy::arc_with_non_send_sync)]
    Self(Arc::new(Mutex::new(inner)))
  }
}

impl Deref for MutexDatabaseRow {
  type Target = Arc<Mutex<DatabaseRow>>;
  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

unsafe impl Sync for MutexDatabaseRow {}

unsafe impl Send for MutexDatabaseRow {}

pub fn mut_row_with_collab<F1: Fn(RowUpdate)>(collab: &Collab, mut_row: F1) {
  collab.with_origin_transact_mut(|txn| {
    if let (Some(data), Some(meta)) = (
      collab.get_map_with_txn(txn, vec![DATA]),
      collab.get_map_with_txn(txn, vec![META]),
    ) {
      let update = RowUpdate::new(txn, &data, &meta);
      mut_row(update);
    }
  });
}
