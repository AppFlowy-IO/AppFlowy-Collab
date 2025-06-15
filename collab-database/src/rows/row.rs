use collab::preclude::{
  Any, ArrayRef, Collab, FillRef, Map, MapExt, MapRef, ReadTxn, ToJson, TransactionMut, YrsValue,
};
use std::borrow::{Borrow, BorrowMut};
use std::collections::HashMap;
use std::ops::{Deref, DerefMut};
use strum::IntoEnumIterator;
use strum_macros::EnumIter;

use collab::preclude::encoding::serde::from_any;
use collab::util::AnyExt;
use collab_entity::CollabType;
use collab_entity::define::DATABASE_ROW_DATA;

use crate::database::timestamp;

use crate::error::DatabaseError;
use crate::rows::{
  Cell, Cells, CellsUpdate, RowChangeSender, RowId, RowMeta, RowMetaUpdate,
  subscribe_row_data_change,
};

use crate::util::encoded_collab;
use crate::views::{OrderObjectPosition, RowOrder};
use crate::{impl_bool_update, impl_i32_update, impl_i64_update};
use collab::core::collab::CollabOptions;
use collab::core::origin::CollabOrigin;
use collab::entity::EncodedCollab;
use serde::{Deserialize, Serialize};
use tracing::{error, trace};
use uuid::Uuid;
use yrs::block::ClientID;

pub type BlockId = i64;

const META: &str = "meta";
const COMMENT: &str = "comment";
pub const LAST_MODIFIED: &str = "last_modified";
pub const CREATED_AT: &str = "created_at";

pub struct DatabaseRow {
  pub row_id: RowId,
  pub collab: Collab,
  pub body: DatabaseRowBody,
}

pub fn default_database_row_from_row(row: Row, client_id: ClientID) -> EncodedCollab {
  let collab = default_database_row_collab(row, client_id);
  collab
    .encode_collab_v1(|_collab| Ok::<_, DatabaseError>(()))
    .unwrap()
}

pub fn default_database_row_data(row: Row, client_id: ClientID) -> EncodedCollab {
  let collab = default_database_row_collab(row, client_id);
  collab
    .encode_collab_v1(|_collab| Ok::<_, DatabaseError>(()))
    .unwrap()
}

pub fn default_database_row_collab(row: Row, client_id: ClientID) -> Collab {
  let row_id = row.id.clone();
  let options = CollabOptions::new(row_id.to_string(), client_id);
  let mut collab = Collab::new_with_options(CollabOrigin::Empty, options).unwrap();
  let _ = DatabaseRowBody::create(row_id.clone(), &mut collab, row);
  collab
}

impl Drop for DatabaseRow {
  fn drop(&mut self) {
    #[cfg(feature = "verbose_log")]
    trace!("DatabaseRow dropped: {}", self.body.row_id);
  }
}

impl DatabaseRow {
  pub fn open(
    row_id: RowId,
    mut collab: Collab,
    change_tx: Option<RowChangeSender>,
  ) -> Result<Self, DatabaseError> {
    let body = DatabaseRowBody::open(row_id.clone(), &mut collab)?;
    if let Some(change_tx) = change_tx {
      subscribe_row_data_change(row_id.clone(), &body.data, change_tx);
    }
    Ok(Self {
      row_id,
      collab,
      body,
    })
  }

  pub fn create(
    row_id: RowId,
    mut collab: Collab,
    change_tx: Option<RowChangeSender>,
    row: Row,
  ) -> Self {
    let body = DatabaseRowBody::create(row_id.clone(), &mut collab, row);
    if let Some(change_tx) = change_tx {
      subscribe_row_data_change(row_id.clone(), &body.data, change_tx);
    }
    Self {
      row_id,
      collab,
      body,
    }
  }

  pub fn encoded_collab(&self) -> Result<EncodedCollab, DatabaseError> {
    let row_encoded = encoded_collab(&self.collab, &CollabType::DatabaseRow)?;
    Ok(row_encoded)
  }

  pub fn validate(&self) -> Result<(), DatabaseError> {
    CollabType::DatabaseRow.validate_require_data(&self.collab)?;
    Ok(())
  }

  pub fn get_row(&self) -> Option<Row> {
    let txn = self.collab.transact();
    row_from_map_ref(&self.body.data, &txn)
  }

  pub fn get_row_meta(&self) -> Option<RowMeta> {
    let txn = self.collab.transact();
    let row_id = Uuid::parse_str(&self.body.row_id).ok()?;
    Some(RowMeta::from_map_ref(&txn, &row_id, &self.body.meta))
  }

  pub fn get_row_detail(&self) -> Option<RowDetail> {
    let txn = self.collab.transact();
    let row = row_from_map_ref(&self.body.data, &txn)?;
    let row_id = Uuid::parse_str(&self.body.row_id).ok()?;
    let meta = RowMeta::from_map_ref(&txn, &row_id, &self.body.meta);
    RowDetail::new(row, meta)
  }

  pub fn get_row_order(&self) -> Option<RowOrder> {
    let txn = self.collab.transact();
    row_order_from_map_ref(&self.body.data, &txn).map(|value| value.0)
  }

  pub fn get_cell(&self, field_id: &str) -> Option<Cell> {
    let txn = self.collab.transact();
    cell_from_map_ref(&self.body.data, &txn, field_id)
  }

  pub fn update<F>(&mut self, f: F)
  where
    F: FnOnce(RowUpdate),
  {
    let data = self.body.data.clone();
    let meta = self.body.meta.clone();
    let mut txn = self.collab.transact_mut();
    let update = RowUpdate::new(&mut txn, data.clone(), meta);
    f(update);

    // updates the row_id in case it has changed
    if let Some(row_id) = row_id_from_map_ref(&txn, &data) {
      self.body.row_id = row_id.clone();
      self.row_id = row_id;
    };
  }

  pub fn update_meta<F>(&mut self, f: F)
  where
    F: FnOnce(RowMetaUpdate),
  {
    let meta = self.body.meta.clone();
    let mut txn = self.collab.transact_mut();
    match Uuid::parse_str(&self.body.row_id) {
      Ok(row_id) => {
        let update = RowMetaUpdate::new(&mut txn, meta, row_id);
        f(update)
      },
      Err(e) => error!("🔴 can't update the row meta: {}", e),
    }
  }
}

impl Deref for DatabaseRow {
  type Target = Collab;

  fn deref(&self) -> &Self::Target {
    &self.collab
  }
}

impl DerefMut for DatabaseRow {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.collab
  }
}

impl Borrow<Collab> for DatabaseRow {
  #[inline]
  fn borrow(&self) -> &Collab {
    &self.collab
  }
}

impl BorrowMut<Collab> for DatabaseRow {
  fn borrow_mut(&mut self) -> &mut Collab {
    &mut self.collab
  }
}

pub struct DatabaseRowBody {
  row_id: RowId,
  data: MapRef,
  #[allow(dead_code)]
  meta: MapRef,
  #[allow(dead_code)]
  comments: ArrayRef,
}

impl DatabaseRowBody {
  pub fn open(row_id: RowId, collab: &mut Collab) -> Result<Self, DatabaseError> {
    CollabType::DatabaseRow.validate_require_data(collab)?;
    Ok(Self::create_with_data(row_id, collab, None))
  }

  pub fn create(row_id: RowId, collab: &mut Collab, row: Row) -> Self {
    Self::create_with_data(row_id, collab, Some(row))
  }

  fn create_with_data(row_id: RowId, collab: &mut Collab, row: Option<Row>) -> Self {
    let mut txn = collab.context.transact_mut();
    let data: MapRef = collab.data.get_or_init(&mut txn, DATABASE_ROW_DATA);
    let meta: MapRef = collab.data.get_or_init(&mut txn, META);
    let comments: ArrayRef = collab.data.get_or_init(&mut txn, COMMENT);
    if let Some(row) = row {
      RowBuilder::new(&mut txn, data.clone(), meta.clone())
        .update(|update| {
          update
            .set_row_id(row.id)
            .set_database_id(row.database_id)
            .set_height(row.height)
            .set_visibility(row.visibility)
            .set_created_at(row.created_at)
            .set_last_modified(row.modified_at)
            .set_cells(row.cells);
        })
        .done();
    }

    DatabaseRowBody {
      row_id,
      data,
      meta,
      comments,
    }
  }

  pub fn update<F>(&self, txn: &mut TransactionMut, modify: F)
  where
    F: FnOnce(RowUpdate),
  {
    let update = RowUpdate::new(txn, self.data.clone(), self.meta.clone());
    modify(update);
  }

  pub fn update_cells<F>(&self, txn: &mut TransactionMut, modify: F)
  where
    F: FnOnce(CellsUpdate),
  {
    let cell_map: MapRef = self.data.get_or_init(txn, ROW_CELLS);
    let update = CellsUpdate::new(txn, &cell_map);
    modify(update);
  }

  pub fn update_id(
    &mut self,
    txn: &mut TransactionMut,
    new_row_id: RowId,
  ) -> Result<(), DatabaseError> {
    self.update(txn, |update| {
      update.set_row_id(new_row_id.clone());
    });
    self.row_id = new_row_id;
    Ok(())
  }

  /// Attempts to get the document id for the row.
  /// Returns None if there is no document.
  pub fn document_id<T: ReadTxn>(&self, txn: &T) -> Result<Option<String>, DatabaseError> {
    let row_uuid = Uuid::parse_str(&self.row_id)?;
    let is_doc_empty_key = meta_id_from_row_id(&row_uuid, RowMetaKey::IsDocumentEmpty);
    let is_doc_empty = self.meta.get(txn, &is_doc_empty_key);
    if let Some(yrs::Out::Any(Any::Bool(is_doc_empty))) = is_doc_empty {
      if !is_doc_empty {
        let doc_id = meta_id_from_row_id(&row_uuid, RowMetaKey::DocumentId);
        return Ok(Some(doc_id));
      }
    }
    Ok(None)
  }

  pub fn cells<T: ReadTxn>(&self, txn: &T) -> Option<Cells> {
    let map = self
      .data
      .get(txn, ROW_CELLS)
      .and_then(|cell| cell.cast::<MapRef>().ok())?;
    let mut cells = Cells::new();
    for (field_id, out) in map.iter(txn) {
      let cell = out.to_json(txn).into_map()?;
      cells.insert(field_id.to_string(), cell);
    }
    Some(cells)
  }

  pub fn get_data(&self) -> &MapRef {
    &self.data
  }

  pub fn get_meta(&self) -> &MapRef {
    &self.meta
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
  pub fn from_collab(collab: &Collab) -> Option<Self> {
    let txn = collab.transact();
    let data: MapRef = collab.get_with_txn(&txn, DATABASE_ROW_DATA)?.cast().ok()?;
    let meta: MapRef = collab.get_with_txn(&txn, META)?.cast().ok()?;
    let row = row_from_map_ref(&data, &txn)?;

    let row_id = Uuid::parse_str(&row.id).ok()?;
    let meta = RowMeta::from_map_ref(&txn, &row_id, &meta);
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
  #[serde(default)]
  pub database_id: String,
  pub cells: Cells,
  pub height: i32,
  #[serde(default = "default_visibility")]
  pub visibility: bool,
  #[serde(deserialize_with = "deserialize_i64")]
  pub created_at: i64,
  #[serde(alias = "last_modified", deserialize_with = "deserialize_i64")]
  pub modified_at: i64,
}

fn deserialize_i64<'de, D>(deserializer: D) -> Result<i64, D::Error>
where
  D: serde::Deserializer<'de>,
{
  use serde::de::{self, Unexpected};
  match serde_json::Value::deserialize(deserializer)? {
    serde_json::Value::Number(num) => num.as_i64().ok_or_else(|| {
      de::Error::invalid_type(
        Unexpected::Other(&format!("{:?}", num)),
        &"a valid i64 number",
      )
    }),
    serde_json::Value::String(s) => s.parse::<i64>().map_err(|_| {
      de::Error::invalid_type(Unexpected::Str(&s), &"a string that can be parsed into i64")
    }),
    other => Err(de::Error::invalid_type(
      Unexpected::Other(&format!("{:?}", other)),
      &"a number or a string that can be parsed into i64",
    )),
  }
}

fn default_visibility() -> bool {
  true
}

#[derive(Clone, Debug, EnumIter)]
pub enum RowMetaKey {
  DocumentId,
  IconId,
  CoverId,
  IsDocumentEmpty,
  AttachmentCount,
}

impl RowMetaKey {
  pub fn as_str(&self) -> &str {
    match self {
      Self::DocumentId => "document_id",
      Self::IconId => "icon_id",
      Self::CoverId => "cover_id",
      Self::IsDocumentEmpty => "is_document_empty",
      Self::AttachmentCount => "attachment_count",
    }
  }
}

const DEFAULT_ROW_HEIGHT: i32 = 60;
impl Row {
  /// Creates a new instance of [Row]
  /// The default height of a [Row] is 60
  /// The default visibility of a [Row] is true
  /// The default created_at of a [Row] is the current timestamp
  pub fn new<R: Into<RowId>>(id: R, database_id: &str) -> Self {
    let timestamp = timestamp();
    Row {
      id: id.into(),
      database_id: database_id.to_string(),
      cells: HashMap::new(),
      height: DEFAULT_ROW_HEIGHT,
      visibility: true,
      created_at: timestamp,
      modified_at: timestamp,
    }
  }

  pub fn empty(row_id: RowId, database_id: &str) -> Self {
    Self {
      id: row_id,
      database_id: database_id.to_string(),
      cells: HashMap::new(),
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
    let update = RowUpdate::new(self.txn, self.map_ref.clone(), self.meta_ref.clone());
    f(update);
    self
  }
  pub fn done(self) {}
}

/// It used to update a [Row]
pub struct RowUpdate<'a, 'b> {
  map_ref: MapRef,
  meta_ref: MapRef,
  txn: &'a mut TransactionMut<'b>,
}

impl<'a, 'b> RowUpdate<'a, 'b> {
  pub fn new(txn: &'a mut TransactionMut<'b>, map_ref: MapRef, meta_ref: MapRef) -> Self {
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

  pub fn set_database_id(self, database_id: String) -> Self {
    self.map_ref.insert(self.txn, ROW_DATABASE_ID, database_id);
    self
  }

  pub fn set_row_id(self, new_row_id: RowId) -> Self {
    let old_row_id = match row_id_from_map_ref(self.txn, &self.map_ref) {
      Some(row_id) => row_id,
      None => {
        // no row id found, so we just insert the new id
        self.map_ref.insert(self.txn, ROW_ID, new_row_id.as_str());
        return self;
      },
    };
    let old_row_uuid = match old_row_id.parse::<Uuid>() {
      Ok(uuid) => uuid,
      Err(err) => {
        error!("uuid parse error: {}, old_row_id: {}", err, old_row_id);
        return self;
      },
    };
    let new_row_uuid = match new_row_id.parse::<Uuid>() {
      Ok(uuid) => uuid,
      Err(err) => {
        error!("uuid parse error: {}, new_row_id: {}", err, new_row_id);
        return self;
      },
    };

    // update to new row id
    self.map_ref.insert(self.txn, ROW_ID, new_row_id.as_str());

    // update meta key derived from new row id
    // this exhaustively iterates over all meta keys
    // so that we can update all meta keys derived from the row id.
    for key in RowMetaKey::iter() {
      let old_meta_key = meta_id_from_row_id(&old_row_uuid, key.clone());
      let old_meta_value = self.meta_ref.remove(self.txn, &old_meta_key);
      let new_meta_key = meta_id_from_row_id(&new_row_uuid, key);
      if let Some(yrs::Out::Any(old_meta_value)) = old_meta_value {
        self.meta_ref.insert(self.txn, new_meta_key, old_meta_value);
      }
    }

    self
  }

  pub fn set_cells(self, cells: Cells) -> Self {
    let cell_map: MapRef = self.map_ref.get_or_init(self.txn, ROW_CELLS);
    Any::from(cells).fill(self.txn, &cell_map).unwrap();
    self
  }

  pub fn update_cells<F>(self, f: F) -> Self
  where
    F: FnOnce(CellsUpdate),
  {
    let cell_map: MapRef = self.map_ref.get_or_init(self.txn, ROW_CELLS);
    let update = CellsUpdate::new(self.txn, &cell_map);
    f(update);
    self
  }

  pub fn get_updated_row(self) -> Option<Row> {
    row_from_map_ref(&self.map_ref, self.txn)
  }
}

pub(crate) const ROW_ID: &str = "id";
pub const ROW_DATABASE_ID: &str = "database_id";
pub(crate) const ROW_VISIBILITY: &str = "visibility";

pub const ROW_HEIGHT: &str = "height";
pub const ROW_CELLS: &str = "cells";

/// Return row id and created_at from a [YrsValue]
pub fn row_id_from_value<T: ReadTxn>(value: YrsValue, txn: &T) -> Option<(String, i64)> {
  let map_ref: MapRef = value.cast().ok()?;
  let id: String = map_ref.get_with_txn(txn, ROW_ID)?;
  let crated_at: i64 = map_ref.get_with_txn(txn, CREATED_AT).unwrap_or_default();
  Some((id, crated_at))
}

/// Return a [RowOrder] and created_at from a [YrsValue]
pub fn row_order_from_value<T: ReadTxn>(value: YrsValue, txn: &T) -> Option<(RowOrder, i64)> {
  let map_ref: MapRef = value.cast().ok()?;
  row_order_from_map_ref(&map_ref, txn)
}

/// Return a [RowOrder] and created_at from a [YrsValue]
pub fn row_order_from_map_ref<T: ReadTxn>(map_ref: &MapRef, txn: &T) -> Option<(RowOrder, i64)> {
  let id = RowId::from(map_ref.get_with_txn::<_, String>(txn, ROW_ID)?);
  let height: i64 = map_ref.get_with_txn(txn, ROW_HEIGHT).unwrap_or(60);
  let crated_at: i64 = map_ref.get_with_txn(txn, CREATED_AT).unwrap_or_default();
  Some((RowOrder::new(id, height as i32), crated_at))
}

/// Return a [Cell] in a [Row] from a [YrsValue]
/// The [Cell] is identified by the field_id
pub fn cell_from_map_ref<T: ReadTxn>(map_ref: &MapRef, txn: &T, field_id: &str) -> Option<Cell> {
  let cells_map_ref: MapRef = map_ref.get_with_txn(txn, ROW_CELLS)?;
  let cell_map_ref: MapRef = cells_map_ref.get_with_txn(txn, field_id)?;
  cell_map_ref.to_json(txn).into_map()
}

pub fn row_id_from_map_ref<T: ReadTxn>(txn: &T, map_ref: &MapRef) -> Option<RowId> {
  let row_id: String = map_ref.get_with_txn(txn, ROW_ID)?;
  Some(RowId::from(row_id))
}

/// Return a [Row] from a [MapRef]
pub fn row_from_map_ref<T: ReadTxn>(map_ref: &MapRef, txn: &T) -> Option<Row> {
  let any = map_ref.to_json(txn);
  match from_any(&any) {
    Ok(row) => Some(row),
    Err(e) => {
      error!("Failed to convert to Row: {}, value:{:#?}", e, any);
      None
    },
  }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateRowParams {
  pub id: RowId,
  pub database_id: String,
  pub cells: Cells,
  pub height: i32,
  pub visibility: bool,
  #[serde(skip)]
  pub row_position: OrderObjectPosition,
  pub created_at: i64,
  #[serde(rename = "last_modified")]
  pub modified_at: i64,
}

pub(crate) struct CreateRowParamsValidator;

impl CreateRowParamsValidator {
  pub(crate) fn validate(mut params: CreateRowParams) -> Result<CreateRowParams, DatabaseError> {
    if params.id.is_empty() {
      return Err(DatabaseError::InvalidRowID("row_id is empty"));
    }

    let timestamp = timestamp();
    if params.created_at == 0 {
      params.created_at = timestamp;
    }
    if params.modified_at == 0 {
      params.modified_at = timestamp;
    }

    Ok(params)
  }
}

impl CreateRowParams {
  pub fn new<T: Into<RowId>>(id: T, database_id: String) -> Self {
    let timestamp = timestamp();
    Self {
      id: id.into(),
      database_id,
      cells: Default::default(),
      height: 60,
      visibility: true,
      row_position: OrderObjectPosition::default(),
      created_at: timestamp,
      modified_at: timestamp,
    }
  }

  pub fn with_cells(mut self, cells: Cells) -> Self {
    self.cells = cells;
    self
  }

  pub fn with_height(mut self, height: i32) -> Self {
    self.height = height;
    self
  }

  pub fn with_visibility(mut self, visibility: bool) -> Self {
    self.visibility = visibility;
    self
  }
  pub fn with_row_position(mut self, row_position: OrderObjectPosition) -> Self {
    self.row_position = row_position;
    self
  }
}

impl From<CreateRowParams> for Row {
  fn from(params: CreateRowParams) -> Self {
    Row {
      id: params.id,
      database_id: params.database_id,
      cells: params.cells,
      height: params.height,
      visibility: params.visibility,
      created_at: params.created_at,
      modified_at: params.modified_at,
    }
  }
}

pub fn mut_row_with_collab<F1: Fn(RowUpdate)>(collab: &mut Collab, mut_row: F1) {
  let mut txn = collab.context.transact_mut();
  if let (Some(YrsValue::YMap(data)), Some(YrsValue::YMap(meta))) = (
    collab.data.get(&txn, DATABASE_ROW_DATA),
    collab.data.get(&txn, META),
  ) {
    let update = RowUpdate::new(&mut txn, data, meta);
    mut_row(update);
  }
}

#[cfg(test)]
mod tests {
  use serde_json::json;

  use super::*;

  #[test]
  fn test_create_row() {
    let input_with_numbers = json!({
      "id": "abcd",
      "database_id": "database_id",
      "cells": {},
      "height": 100,
      "visibility": false,
      "created_at": 1678901234,
      "modified_at": 1678901234
    });
    let row: Row = serde_json::from_value(input_with_numbers)
      .expect("Failed to deserialize row with number as i64");
    assert_eq!(row.created_at, 1678901234);
    let input_with_string = json!({
      "id": "abcd",
      "database_id": "database_id",
      "cells": {},
      "height": 100,
      "visibility": false,
      "created_at": "1678901234",
      "modified_at": "1678901234"
    });
    let row: Row = serde_json::from_value(input_with_string)
      .expect("Failed to deserialize row with string as i64");
    assert_eq!(row.created_at, 1678901234);
  }
}
