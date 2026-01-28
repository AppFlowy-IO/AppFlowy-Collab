use crate::database::rows::comment::{
  COMMENT_CONTENT, COMMENT_IS_RESOLVED, COMMENT_REACTIONS, COMMENT_RESOLVED_AT,
  COMMENT_RESOLVED_BY, COMMENT_UPDATED_AT, RowComment,
};
use crate::preclude::{
  Any, Collab, FillRef, Map, MapExt, MapRef, ReadTxn, ToJson, TransactionMut, YrsValue,
};
use std::borrow::{Borrow, BorrowMut};
use std::collections::HashMap;
use std::ops::{Deref, DerefMut};
use strum::IntoEnumIterator;
use strum_macros::EnumIter;
#[cfg(feature = "verbose_log")]
use tracing::trace;

use crate::entity::CollabType;
use crate::entity::define::DATABASE_ROW_DATA;
use crate::entity::uuid_validation::{DatabaseId, RowId};
use crate::preclude::encoding::serde::from_any;
use crate::util::AnyExt;

use crate::database::database::timestamp;

use super::row_observer::subscribe_row_comment_change;
use crate::database::rows::{
  Cell, Cells, CellsUpdate, RowChangeSender, RowMeta, RowMetaUpdate, subscribe_row_data_change,
  subscribe_row_meta_change,
};
use crate::error::CollabError;

use crate::core::collab::CollabOptions;
use crate::core::origin::CollabOrigin;
use crate::database::util::encoded_collab;
use crate::database::views::{OrderObjectPosition, RowOrder};
use crate::entity::EncodedCollab;
use serde::{Deserialize, Serialize};
use tracing::error;
use uuid::Uuid;
use yrs::block::ClientID;

pub type BlockId = i64;

const META: &str = "meta";
const COMMENT: &str = "comment";
const ROW_REACTIONS: &str = "row_reactions";
pub const LAST_MODIFIED: &str = "last_modified";
pub const CREATED_AT: &str = "created_at";
pub const CREATED_BY: &str = "created_by";

pub struct DatabaseRow {
  pub row_id: RowId,
  pub collab: Collab,
  pub body: DatabaseRowBody,
}

pub fn default_database_row_from_row(row: Row, client_id: ClientID) -> EncodedCollab {
  let collab = default_database_row_collab(row, client_id);
  collab
    .encode_collab_v1(|_collab| Ok::<_, CollabError>(()))
    .unwrap()
}

pub fn default_database_row_data(row: Row, client_id: ClientID) -> EncodedCollab {
  let collab = default_database_row_collab(row, client_id);
  collab
    .encode_collab_v1(|_collab| Ok::<_, CollabError>(()))
    .unwrap()
}

pub fn default_database_row_collab(row: Row, client_id: ClientID) -> Collab {
  let row_id = row.id;
  let options = CollabOptions::new(row_id, client_id);
  let mut collab = Collab::new_with_options(CollabOrigin::Empty, options).unwrap();
  let _ = DatabaseRowBody::create(row_id, &mut collab, row);
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
  ) -> Result<Self, CollabError> {
    let body = DatabaseRowBody::open(row_id, &mut collab)?;
    if let Some(change_tx) = change_tx {
      let origin = collab.origin().clone();
      let meta_change_tx = change_tx.clone();
      let comment_change_tx = change_tx.clone();
      subscribe_row_data_change(origin.clone(), row_id, &body.data, change_tx);
      subscribe_row_meta_change(origin.clone(), row_id, &body.meta, meta_change_tx);
      subscribe_row_comment_change(origin, row_id, &body.comments, comment_change_tx);
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
    let body = DatabaseRowBody::create(row_id, &mut collab, row);
    if let Some(change_tx) = change_tx {
      let origin = collab.origin().clone();
      let meta_change_tx = change_tx.clone();
      let comment_change_tx = change_tx.clone();
      subscribe_row_data_change(origin.clone(), row_id, &body.data, change_tx);
      subscribe_row_meta_change(origin.clone(), row_id, &body.meta, meta_change_tx);
      subscribe_row_comment_change(origin, row_id, &body.comments, comment_change_tx);
    }
    Self {
      row_id,
      collab,
      body,
    }
  }

  pub fn encoded_collab(&self) -> Result<EncodedCollab, CollabError> {
    let row_encoded = encoded_collab(&self.collab, &CollabType::DatabaseRow)?;
    Ok(row_encoded)
  }

  pub fn validate(&self) -> Result<(), CollabError> {
    CollabType::DatabaseRow.validate_require_data(&self.collab)?;
    Ok(())
  }

  pub fn get_row(&self) -> Option<Row> {
    let txn = self.collab.transact();
    row_from_map_ref(&self.body.data, &txn)
  }

  pub fn get_row_meta(&self) -> Option<RowMeta> {
    let txn = self.collab.transact();
    let row_id = self.body.row_id;
    Some(RowMeta::from_map_ref(&txn, &row_id, &self.body.meta))
  }

  pub fn get_row_detail(&self) -> Option<RowDetail> {
    let txn = self.collab.transact();
    let row = row_from_map_ref(&self.body.data, &txn)?;
    let row_id = self.body.row_id;
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
      self.body.row_id = row_id;
      self.row_id = row_id;
    };
  }

  pub fn update_meta<F>(&mut self, f: F)
  where
    F: FnOnce(RowMetaUpdate),
  {
    let meta = self.body.meta.clone();
    let mut txn = self.collab.transact_mut();
    let row_id = self.body.row_id;
    let update = RowMetaUpdate::new(&mut txn, meta, row_id);
    f(update)
  }

  // ==================== Comment Methods ====================

  /// Get all comments for this row
  pub fn get_comments(&self) -> Vec<RowComment> {
    let txn = self.collab.transact();
    self.body.get_all_comments(&txn)
  }

  /// Get a specific comment by ID
  pub fn get_comment(&self, comment_id: &str) -> Option<RowComment> {
    let txn = self.collab.transact();
    self.body.get_comment(&txn, comment_id)
  }

  /// Add a new comment
  pub fn add_comment(&mut self, comment: RowComment) -> String {
    let mut txn = self.collab.transact_mut();
    self.body.add_comment(&mut txn, comment)
  }

  /// Update comment content
  pub fn update_comment_content(&mut self, comment_id: &str, content: String) -> bool {
    let mut txn = self.collab.transact_mut();
    self
      .body
      .update_comment_content(&mut txn, comment_id, content)
  }

  /// Delete a comment
  pub fn delete_comment(&mut self, comment_id: &str) -> bool {
    let mut txn = self.collab.transact_mut();
    self.body.delete_comment(&mut txn, comment_id)
  }

  /// Set the resolved status of a comment
  pub fn set_comment_resolved(
    &mut self,
    comment_id: &str,
    is_resolved: bool,
    resolved_by: Option<String>,
  ) -> bool {
    let mut txn = self.collab.transact_mut();
    self
      .body
      .set_comment_resolved(&mut txn, comment_id, is_resolved, resolved_by)
  }

  /// Add a reaction to a comment
  pub fn add_comment_reaction(&mut self, comment_id: &str, emoji: &str, user_id: i64) -> bool {
    let mut txn = self.collab.transact_mut();
    self
      .body
      .add_comment_reaction(&mut txn, comment_id, emoji, user_id)
  }

  /// Remove a reaction from a comment
  pub fn remove_comment_reaction(&mut self, comment_id: &str, emoji: &str, user_id: i64) -> bool {
    let mut txn = self.collab.transact_mut();
    self
      .body
      .remove_comment_reaction(&mut txn, comment_id, emoji, user_id)
  }

  /// Get comment count
  pub fn get_comment_count(&self) -> usize {
    let txn = self.collab.transact();
    self.body.get_comment_count(&txn)
  }

  /// Get replies to a specific comment
  pub fn get_comment_replies(&self, parent_comment_id: &str) -> Vec<RowComment> {
    let txn = self.collab.transact();
    self.body.get_comment_replies(&txn, parent_comment_id)
  }

  /// Add a reaction to the row
  pub fn add_row_reaction(&mut self, emoji: &str, user_id: i64) {
    let mut txn = self.collab.transact_mut();
    self.body.add_row_reaction(&mut txn, emoji, user_id);
  }

  /// Remove a reaction from the row
  pub fn remove_row_reaction(&mut self, emoji: &str, user_id: i64) {
    let mut txn = self.collab.transact_mut();
    self.body.remove_row_reaction(&mut txn, emoji, user_id);
  }

  /// Get all reactions for this row
  pub fn get_row_reactions(&self) -> HashMap<String, Vec<i64>> {
    let txn = self.collab.transact();
    self.body.get_row_reactions(&txn)
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
  meta: MapRef,
  /// Comments stored as MapRef keyed by comment_id for O(1) lookup
  comments: MapRef,
}

impl DatabaseRowBody {
  pub fn open(row_id: RowId, collab: &mut Collab) -> Result<Self, CollabError> {
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
    // Initialize comments as MapRef for O(1) lookup by comment_id
    let comments: MapRef = collab.data.get_or_init(&mut txn, COMMENT);
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
            .set_created_by_if_not_none(row.created_by)
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
  ) -> Result<(), CollabError> {
    self.update(txn, |update| {
      update.set_row_id(new_row_id);
    });
    self.row_id = new_row_id;
    Ok(())
  }

  /// Attempts to get the document id for the row.
  /// Returns None if there is no document.
  pub fn document_id<T: ReadTxn>(&self, txn: &T) -> Result<Option<String>, CollabError> {
    let row_uuid = self.row_id;
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

  pub fn get_comments(&self) -> &MapRef {
    &self.comments
  }

  // ==================== Comment CRUD Methods ====================

  /// Get all comments for this row
  pub fn get_all_comments<T: ReadTxn>(&self, txn: &T) -> Vec<RowComment> {
    let mut comments = Vec::new();
    for (_key, value) in self.comments.iter(txn) {
      if let Ok(comment_map) = value.cast::<MapRef>() {
        if let Some(comment) = RowComment::from_map_ref(&comment_map, txn) {
          comments.push(comment);
        }
      }
    }
    // Sort by created_at in ascending order
    comments.sort_by(|a, b| a.created_at.cmp(&b.created_at));
    comments
  }

  /// Get a specific comment by ID
  pub fn get_comment<T: ReadTxn>(&self, txn: &T, comment_id: &str) -> Option<RowComment> {
    let comment_map: MapRef = self.comments.get_with_txn(txn, comment_id)?;
    RowComment::from_map_ref(&comment_map, txn)
  }

  /// Add a new comment and return its ID
  pub fn add_comment(&self, txn: &mut TransactionMut, comment: RowComment) -> String {
    let comment_id = comment.id.clone();
    let comment_map: MapRef = self.comments.get_or_init(txn, comment_id.as_str());
    comment.fill_map_ref(txn, &comment_map);
    comment_id
  }

  /// Update the content of an existing comment
  pub fn update_comment_content(
    &self,
    txn: &mut TransactionMut,
    comment_id: &str,
    content: String,
  ) -> bool {
    if let Some(comment_map) = self
      .comments
      .get(txn, comment_id)
      .and_then(|v| v.cast::<MapRef>().ok())
    {
      comment_map.insert(txn, COMMENT_CONTENT, content);
      comment_map.insert(txn, COMMENT_UPDATED_AT, Any::BigInt(timestamp()));
      true
    } else {
      false
    }
  }

  /// Delete a comment by ID
  pub fn delete_comment(&self, txn: &mut TransactionMut, comment_id: &str) -> bool {
    self.comments.remove(txn, comment_id).is_some()
  }

  /// Set the resolved status of a comment
  pub fn set_comment_resolved(
    &self,
    txn: &mut TransactionMut,
    comment_id: &str,
    is_resolved: bool,
    resolved_by: Option<String>,
  ) -> bool {
    if let Some(comment_map) = self
      .comments
      .get(txn, comment_id)
      .and_then(|v| v.cast::<MapRef>().ok())
    {
      comment_map.insert(txn, COMMENT_IS_RESOLVED, is_resolved);
      let now = timestamp();
      comment_map.insert(txn, COMMENT_UPDATED_AT, Any::BigInt(now));

      if is_resolved {
        if let Some(resolved_by) = resolved_by {
          comment_map.insert(txn, COMMENT_RESOLVED_BY, resolved_by);
        }
        comment_map.insert(txn, COMMENT_RESOLVED_AT, Any::BigInt(now));
      } else {
        // Clear resolved fields if unresolving
        comment_map.remove(txn, COMMENT_RESOLVED_BY);
        comment_map.remove(txn, COMMENT_RESOLVED_AT);
      }
      true
    } else {
      false
    }
  }

  /// Add a reaction to a comment
  pub fn add_comment_reaction(
    &self,
    txn: &mut TransactionMut,
    comment_id: &str,
    emoji: &str,
    user_id: i64,
  ) -> bool {
    if let Some(comment_map) = self
      .comments
      .get(txn, comment_id)
      .and_then(|v| v.cast::<MapRef>().ok())
    {
      // Get current reactions
      let reactions_str: Option<String> = comment_map.get_with_txn(txn, COMMENT_REACTIONS);
      let mut reactions: std::collections::HashMap<String, Vec<i64>> = reactions_str
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();

      // Add user to the emoji's user list if not already present
      let user_list = reactions.entry(emoji.to_string()).or_default();
      if !user_list.contains(&user_id) {
        user_list.push(user_id);
      }

      // Save back to the map
      if let Ok(reactions_json) = serde_json::to_string(&reactions) {
        comment_map.insert(txn, COMMENT_REACTIONS, reactions_json);
      }
      comment_map.insert(txn, COMMENT_UPDATED_AT, Any::BigInt(timestamp()));
      true
    } else {
      false
    }
  }

  /// Remove a reaction from a comment
  pub fn remove_comment_reaction(
    &self,
    txn: &mut TransactionMut,
    comment_id: &str,
    emoji: &str,
    user_id: i64,
  ) -> bool {
    if let Some(comment_map) = self
      .comments
      .get(txn, comment_id)
      .and_then(|v| v.cast::<MapRef>().ok())
    {
      // Get current reactions
      let reactions_str: Option<String> = comment_map.get_with_txn(txn, COMMENT_REACTIONS);
      let mut reactions: std::collections::HashMap<String, Vec<i64>> = reactions_str
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();

      // Remove user from the emoji's user list
      if let Some(user_list) = reactions.get_mut(emoji) {
        user_list.retain(|&id| id != user_id);
        // Remove the emoji entry if no users left
        if user_list.is_empty() {
          reactions.remove(emoji);
        }
      }

      // Save back to the map
      if let Ok(reactions_json) = serde_json::to_string(&reactions) {
        comment_map.insert(txn, COMMENT_REACTIONS, reactions_json);
      }
      comment_map.insert(txn, COMMENT_UPDATED_AT, Any::BigInt(timestamp()));
      true
    } else {
      false
    }
  }

  /// Get the count of comments for this row
  pub fn get_comment_count<T: ReadTxn>(&self, txn: &T) -> usize {
    self.comments.len(txn) as usize
  }

  /// Get replies to a specific comment
  pub fn get_comment_replies<T: ReadTxn>(
    &self,
    txn: &T,
    parent_comment_id: &str,
  ) -> Vec<RowComment> {
    self
      .get_all_comments(txn)
      .into_iter()
      .filter(|c| c.parent_comment_id.as_deref() == Some(parent_comment_id))
      .collect()
  }

  // ==================== Row Reaction Methods ====================

  /// Add a reaction to the row
  pub fn add_row_reaction(&self, txn: &mut TransactionMut, emoji: &str, user_id: i64) {
    // Get current reactions from meta
    let reactions_str: Option<String> = self.meta.get_with_txn(txn, ROW_REACTIONS);
    let mut reactions: HashMap<String, Vec<i64>> = reactions_str
      .and_then(|s| serde_json::from_str(&s).ok())
      .unwrap_or_default();

    // Add user to the emoji's user list if not already present
    let user_list = reactions.entry(emoji.to_string()).or_default();
    if !user_list.contains(&user_id) {
      user_list.push(user_id);
    }

    // Save back to the meta map
    if let Ok(reactions_json) = serde_json::to_string(&reactions) {
      self.meta.insert(txn, ROW_REACTIONS, reactions_json);
    }
  }

  /// Remove a reaction from the row
  pub fn remove_row_reaction(&self, txn: &mut TransactionMut, emoji: &str, user_id: i64) {
    // Get current reactions from meta
    let reactions_str: Option<String> = self.meta.get_with_txn(txn, ROW_REACTIONS);
    let mut reactions: HashMap<String, Vec<i64>> = reactions_str
      .and_then(|s| serde_json::from_str(&s).ok())
      .unwrap_or_default();

    // Remove user from the emoji's user list
    if let Some(user_list) = reactions.get_mut(emoji) {
      user_list.retain(|&id| id != user_id);
      // Remove the emoji entry if no users left
      if user_list.is_empty() {
        reactions.remove(emoji);
      }
    }

    // Save back to the meta map
    if let Ok(reactions_json) = serde_json::to_string(&reactions) {
      self.meta.insert(txn, ROW_REACTIONS, reactions_json);
    }
  }

  /// Get all reactions for this row
  pub fn get_row_reactions<T: ReadTxn>(&self, txn: &T) -> HashMap<String, Vec<i64>> {
    let reactions_str: Option<String> = self.meta.get_with_txn(txn, ROW_REACTIONS);
    reactions_str
      .and_then(|s| serde_json::from_str(&s).ok())
      .unwrap_or_default()
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
    let document_id = meta_id_from_row_id(&row.id, RowMetaKey::DocumentId);
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

    let row_uuid = row.id;
    let meta = RowMeta::from_map_ref(&txn, &row_uuid, &meta);
    let row_document_id = meta_id_from_row_id(&row_uuid, RowMetaKey::DocumentId);
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
  pub database_id: DatabaseId,
  pub cells: Cells,
  pub height: i32,
  #[serde(default = "default_visibility")]
  pub visibility: bool,
  #[serde(deserialize_with = "deserialize_i64")]
  pub created_at: i64,
  #[serde(alias = "last_modified", deserialize_with = "deserialize_i64")]
  pub modified_at: i64,
  /// The user ID who created this row
  #[serde(default, deserialize_with = "deserialize_option_i64")]
  pub created_by: Option<i64>,
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

fn deserialize_option_i64<'de, D>(deserializer: D) -> Result<Option<i64>, D::Error>
where
  D: serde::Deserializer<'de>,
{
  use serde::de::{self, Unexpected};
  match serde_json::Value::deserialize(deserializer)? {
    serde_json::Value::Null => Ok(None),
    serde_json::Value::Number(num) => num.as_i64().map(Some).ok_or_else(|| {
      de::Error::invalid_type(
        Unexpected::Other(&format!("{:?}", num)),
        &"a valid i64 number or null",
      )
    }),
    serde_json::Value::String(s) => s.parse::<i64>().map(Some).map_err(|_| {
      de::Error::invalid_type(
        Unexpected::Str(&s),
        &"a string that can be parsed into i64 or null",
      )
    }),
    other => Err(de::Error::invalid_type(
      Unexpected::Other(&format!("{:?}", other)),
      &"a number, a string that can be parsed into i64, or null",
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
  pub fn new(id: RowId, database_id: DatabaseId) -> Self {
    let timestamp = timestamp();
    Row {
      id,
      database_id,
      cells: HashMap::new(),
      height: DEFAULT_ROW_HEIGHT,
      visibility: true,
      created_at: timestamp,
      modified_at: timestamp,
      created_by: None,
    }
  }

  /// Creates a new instance of [Row] with the creator's user ID
  pub fn new_with_creator(id: RowId, database_id: DatabaseId, uid: i64) -> Self {
    let timestamp = timestamp();
    Row {
      id,
      database_id,
      cells: HashMap::new(),
      height: DEFAULT_ROW_HEIGHT,
      visibility: true,
      created_at: timestamp,
      modified_at: timestamp,
      created_by: Some(uid),
    }
  }

  pub fn empty(row_id: RowId, database_id: DatabaseId) -> Self {
    Self {
      id: row_id,
      database_id,
      cells: HashMap::new(),
      height: DEFAULT_ROW_HEIGHT,
      visibility: true,
      created_at: 0,
      modified_at: 0,
      created_by: None,
    }
  }

  pub fn is_empty(&self) -> bool {
    self.cells.is_empty()
  }

  pub fn document_id(&self) -> String {
    meta_id_from_row_id(&self.id, RowMetaKey::DocumentId)
  }

  pub fn icon_id(&self) -> String {
    meta_id_from_row_id(&self.id, RowMetaKey::IconId)
  }

  pub fn cover_id(&self) -> String {
    meta_id_from_row_id(&self.id, RowMetaKey::CoverId)
  }
}

pub fn database_row_document_id_from_row_id(row_id: &RowId) -> RowId {
  Uuid::new_v5(row_id, RowMetaKey::DocumentId.as_str().as_bytes())
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

  pub fn set_visibility(self, value: bool) -> Self {
    self.map_ref.insert(self.txn, ROW_VISIBILITY, value);
    self
  }

  pub fn set_visibility_if_not_none(self, value: Option<bool>) -> Self {
    if let Some(value) = value {
      self.map_ref.insert(self.txn, ROW_VISIBILITY, value);
    }
    self
  }

  pub fn set_height(self, value: i32) -> Self {
    self
      .map_ref
      .insert(self.txn, ROW_HEIGHT, Any::BigInt(value as i64));
    self
  }

  pub fn set_height_at_if_not_none(self, value: Option<i32>) -> Self {
    if let Some(value) = value {
      self
        .map_ref
        .insert(self.txn, ROW_HEIGHT, Any::BigInt(value as i64));
    }
    self
  }

  pub fn set_created_at(self, value: i64) -> Self {
    self
      .map_ref
      .insert(self.txn, CREATED_AT, Any::BigInt(value));
    self
  }

  pub fn set_created_at_if_not_none(self, value: Option<i64>) -> Self {
    if let Some(value) = value {
      self
        .map_ref
        .insert(self.txn, CREATED_AT, Any::BigInt(value));
    }
    self
  }

  pub fn set_last_modified(self, value: i64) -> Self {
    self
      .map_ref
      .insert(self.txn, LAST_MODIFIED, Any::BigInt(value));
    self
  }

  pub fn set_last_modified_if_not_none(self, value: Option<i64>) -> Self {
    if let Some(value) = value {
      self
        .map_ref
        .insert(self.txn, LAST_MODIFIED, Any::BigInt(value));
    }
    self
  }

  pub fn set_created_by(self, value: i64) -> Self {
    self
      .map_ref
      .insert(self.txn, CREATED_BY, Any::BigInt(value));
    self
  }

  pub fn set_created_by_if_not_none(self, value: Option<i64>) -> Self {
    if let Some(value) = value {
      self
        .map_ref
        .insert(self.txn, CREATED_BY, Any::BigInt(value));
    }
    self
  }

  pub fn set_database_id(self, database_id: DatabaseId) -> Self {
    self
      .map_ref
      .insert(self.txn, ROW_DATABASE_ID, database_id.to_string());
    self
  }

  pub fn set_row_id(self, new_row_id: RowId) -> Self {
    let old_row_id = match row_id_from_map_ref(self.txn, &self.map_ref) {
      Some(row_id) => row_id,
      None => {
        // no row id found, so we just insert the new id
        self
          .map_ref
          .insert(self.txn, ROW_ID, new_row_id.to_string());
        return self;
      },
    };
    let old_row_uuid = old_row_id;
    let new_row_uuid = new_row_id;

    // update to new row id
    self
      .map_ref
      .insert(self.txn, ROW_ID, new_row_id.to_string());

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
  let row_id_str: String = map_ref.get_with_txn(txn, ROW_ID)?;
  let id = uuid::Uuid::parse_str(&row_id_str).ok()?;
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
  uuid::Uuid::parse_str(&row_id).ok()
}

/// Return a [Row] from a [MapRef]
pub fn row_from_map_ref<T: ReadTxn>(map_ref: &MapRef, txn: &T) -> Option<Row> {
  let any = map_ref.to_json(txn);
  match from_any::<Row>(&any) {
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
  pub database_id: DatabaseId,
  pub cells: Cells,
  pub height: i32,
  pub visibility: bool,
  #[serde(skip)]
  pub row_position: OrderObjectPosition,
  pub created_at: i64,
  #[serde(rename = "last_modified")]
  pub modified_at: i64,
  pub row_meta: Option<RowMeta>,
  /// The user ID who created this row
  #[serde(default)]
  pub created_by: Option<i64>,
}

pub(crate) struct CreateRowParamsValidator;

impl CreateRowParamsValidator {
  pub(crate) fn validate(mut params: CreateRowParams) -> Result<CreateRowParams, CollabError> {
    // RowId is always valid since it's a UUID type

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
  pub fn new(id: RowId, database_id: DatabaseId) -> Self {
    let timestamp = timestamp();
    Self {
      id,
      database_id,
      cells: Default::default(),
      height: 60,
      visibility: true,
      row_position: OrderObjectPosition::default(),
      created_at: timestamp,
      modified_at: timestamp,
      row_meta: None,
      created_by: None,
    }
  }

  /// Creates a new instance of [CreateRowParams] with the creator's user ID
  pub fn new_with_creator(id: RowId, database_id: DatabaseId, uid: i64) -> Self {
    let timestamp = timestamp();
    Self {
      id,
      database_id,
      cells: Default::default(),
      height: 60,
      visibility: true,
      row_position: OrderObjectPosition::default(),
      created_at: timestamp,
      modified_at: timestamp,
      row_meta: None,
      created_by: Some(uid),
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

  pub fn with_row_meta(mut self, row_meta: Option<RowMeta>) -> Self {
    self.row_meta = row_meta;
    self
  }

  pub fn with_created_by(mut self, created_by: Option<i64>) -> Self {
    self.created_by = created_by;
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
      created_by: params.created_by,
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
    let row_id = "550e8400-e29b-41d4-a716-446655440000";
    let database_id = "650e8400-e29b-41d4-a716-446655440001";

    let input_with_numbers = json!({
      "id": row_id,
      "database_id": database_id,
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
      "id": row_id,
      "database_id": database_id,
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

  #[test]
  fn test_row_created_by_serialization() {
    use crate::preclude::encoding::serde::from_any;

    let row_id = "550e8400-e29b-41d4-a716-446655440000";
    let database_id = "650e8400-e29b-41d4-a716-446655440001";

    // Test with created_by as number
    let input_with_created_by = json!({
      "id": row_id,
      "database_id": database_id,
      "cells": {},
      "height": 100,
      "visibility": true,
      "created_at": 1678901234,
      "modified_at": 1678901234,
      "created_by": 12345
    });
    let row: Row = serde_json::from_value(input_with_created_by)
      .expect("Failed to deserialize row with created_by");
    assert_eq!(row.created_by, Some(12345));

    // Test without created_by (should default to None)
    let input_without_created_by = json!({
      "id": row_id,
      "database_id": database_id,
      "cells": {},
      "height": 100,
      "visibility": true,
      "created_at": 1678901234,
      "modified_at": 1678901234
    });
    let row: Row = serde_json::from_value(input_without_created_by)
      .expect("Failed to deserialize row without created_by");
    assert_eq!(row.created_by, None);

    // Test deserialization from yrs Any type (simulating how it's stored in collab)
    let any_value = Any::Map(std::sync::Arc::new(
      [
        ("id".into(), Any::String(row_id.into())),
        ("database_id".into(), Any::String(database_id.into())),
        ("cells".into(), Any::Map(std::sync::Arc::new([].into()))),
        ("height".into(), Any::BigInt(100)),
        ("visibility".into(), Any::Bool(true)),
        ("created_at".into(), Any::BigInt(1678901234)),
        ("last_modified".into(), Any::BigInt(1678901234)),
        ("created_by".into(), Any::BigInt(12345)),
      ]
      .into(),
    ));

    let row: Row = from_any(&any_value).expect("Failed to deserialize row from Any");
    println!("Row created_by from Any: {:?}", row.created_by);
    assert_eq!(
      row.created_by,
      Some(12345),
      "created_by should be Some(12345) when deserialized from Any::BigInt"
    );
  }

  #[test]
  fn test_row_created_by_collab_roundtrip() {
    use crate::core::collab::CollabOptions;
    use crate::core::origin::CollabOrigin;
    use uuid::Uuid;

    let row_id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
    let database_id = Uuid::parse_str("650e8400-e29b-41d4-a716-446655440001").unwrap();
    let user_id: i64 = 12345;

    // Create a row with created_by set
    let row = Row::new_with_creator(row_id, database_id, user_id);
    assert_eq!(row.created_by, Some(user_id));

    // Create a collab and store the row
    let options = CollabOptions::new(row_id, 1);
    let collab = Collab::new_with_options(CollabOrigin::Empty, options).unwrap();
    let database_row = DatabaseRow::create(row_id, collab, None, row.clone());

    // Read the row back from the collab
    let retrieved_row = database_row.get_row();
    assert!(
      retrieved_row.is_some(),
      "Should be able to retrieve row from collab"
    );

    let retrieved_row = retrieved_row.unwrap();
    println!("Original row created_by: {:?}", row.created_by);
    println!("Retrieved row created_by: {:?}", retrieved_row.created_by);

    assert_eq!(
      retrieved_row.created_by,
      Some(user_id),
      "created_by should persist after storing and retrieving from collab"
    );
  }

  #[test]
  fn test_row_created_by_encoded_collab_roundtrip() {
    use crate::core::collab::CollabOptions;
    use crate::core::origin::CollabOrigin;
    use uuid::Uuid;
    use yrs::updates::decoder::Decode;

    let row_id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
    let database_id = Uuid::parse_str("650e8400-e29b-41d4-a716-446655440001").unwrap();
    let user_id: i64 = 12345;
    let client_id: yrs::block::ClientID = 1;

    // Create a row with created_by set
    let row = Row::new_with_creator(row_id, database_id, user_id);
    assert_eq!(row.created_by, Some(user_id));

    // Convert row to EncodedCollab (simulates storage)
    let encoded_collab = default_database_row_from_row(row.clone(), client_id);
    println!(
      "Encoded collab state_vector len: {}",
      encoded_collab.state_vector.len()
    );
    println!(
      "Encoded collab doc_state len: {}",
      encoded_collab.doc_state.len()
    );

    // Decode the collab (simulates loading from storage)
    // Use CollabOptions with the same client_id for consistency
    let options = CollabOptions::new(row_id, client_id);
    let mut collab =
      Collab::new_with_options(CollabOrigin::Empty, options).expect("Failed to create new collab");

    // Apply the encoded state to the new collab
    let update =
      yrs::Update::decode_v1(&encoded_collab.doc_state).expect("Failed to decode doc_state");
    {
      let mut txn = collab.transact_mut();
      txn.apply_update(update).expect("Failed to apply update");
    }

    // Open as DatabaseRow
    let database_row = DatabaseRow::open(row_id, collab, None).expect("Failed to open DatabaseRow");

    // Read the row back
    let retrieved_row = database_row.get_row();
    assert!(
      retrieved_row.is_some(),
      "Should be able to retrieve row from decoded collab"
    );

    let retrieved_row = retrieved_row.unwrap();
    println!("Original row created_by: {:?}", row.created_by);
    println!(
      "Retrieved row (after encode/decode) created_by: {:?}",
      retrieved_row.created_by
    );

    assert_eq!(
      retrieved_row.created_by,
      Some(user_id),
      "created_by should persist after encode/decode cycle (simulating storage)"
    );
  }
}
