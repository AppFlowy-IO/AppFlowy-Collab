use collab::preclude::{Map, MapExt, MapRef, ReadTxn, TransactionMut};
use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use uuid::Uuid;

use crate::{
  entity::FileUploadType,
  rows::{RowMetaKey, meta_id_from_row_id},
};

pub struct RowMetaUpdate<'a, 'b> {
  #[allow(dead_code)]
  map_ref: MapRef,

  #[allow(dead_code)]
  txn: &'a mut TransactionMut<'b>,

  row_id: Uuid,
}

impl<'a, 'b> RowMetaUpdate<'a, 'b> {
  pub fn new(txn: &'a mut TransactionMut<'b>, map_ref: MapRef, row_id: Uuid) -> Self {
    Self {
      map_ref,
      txn,
      row_id,
    }
  }
  pub fn insert_icon_if_not_none(self, icon_url: Option<String>) -> Self {
    if let Some(icon) = icon_url {
      self.insert_icon(&icon)
    } else {
      self
    }
  }

  pub fn insert_cover_if_not_none(self, cover: Option<RowCover>) -> Self {
    if let Some(cover) = cover {
      self.insert_cover(&cover)
    } else {
      self
    }
  }

  pub fn update_is_document_empty_if_not_none(self, is_document_empty: Option<bool>) -> Self {
    if let Some(is_empty) = is_document_empty {
      self.update_is_document_empty(is_empty)
    } else {
      self
    }
  }

  pub fn update_attachment_count_if_not_none(self, attachment_count: Option<i64>) -> Self {
    if let Some(attachment_count) = attachment_count {
      self.update_attachment_count(attachment_count)
    } else {
      self
    }
  }

  pub fn insert_icon(self, icon_url: &str) -> Self {
    let icon_id = meta_id_from_row_id(&self.row_id, RowMetaKey::IconId);
    self.map_ref.insert(self.txn, icon_id, icon_url);
    self
  }

  pub fn insert_cover(self, cover: &RowCover) -> Self {
    let cover_id = meta_id_from_row_id(&self.row_id, RowMetaKey::CoverId);
    self.map_ref.insert(
      self.txn,
      cover_id,
      serde_json::to_string(cover).unwrap_or_default(),
    );
    self
  }

  pub fn update_is_document_empty(self, is_document_empty: bool) -> Self {
    let is_document_empty_id = meta_id_from_row_id(&self.row_id, RowMetaKey::IsDocumentEmpty);
    self
      .map_ref
      .insert(self.txn, is_document_empty_id, is_document_empty);
    self
  }

  pub fn update_attachment_count(self, attachment_count: i64) -> Self {
    let attachment_count_id = meta_id_from_row_id(&self.row_id, RowMetaKey::AttachmentCount);
    self
      .map_ref
      .insert(self.txn, attachment_count_id, attachment_count);
    self
  }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct RowCover {
  pub data: String,
  pub upload_type: FileUploadType,
  pub cover_type: CoverType,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize_repr, Deserialize_repr)]
#[repr(u8)]
pub enum CoverType {
  #[default]
  ColorCover = 0,
  FileCover = 1,
  AssetCover = 2,
  GradientCover = 3,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RowMeta {
  pub icon_url: Option<String>,
  pub cover: Option<RowCover>,
  pub is_document_empty: bool,
  pub attachment_count: i64,
}

impl RowMeta {
  #[allow(dead_code)]
  pub(crate) fn empty() -> Self {
    Self {
      icon_url: None,
      cover: None,
      is_document_empty: true,
      attachment_count: 0,
    }
  }

  pub(crate) fn from_map_ref<T: ReadTxn>(txn: &T, row_id: &Uuid, map_ref: &MapRef) -> Self {
    let cover_data: String = map_ref
      .get_with_txn(txn, &meta_id_from_row_id(row_id, RowMetaKey::CoverId))
      .unwrap_or_default();

    Self {
      icon_url: map_ref.get_with_txn(txn, &meta_id_from_row_id(row_id, RowMetaKey::IconId)),
      cover: serde_json::from_str(&cover_data).unwrap_or(None),
      is_document_empty: map_ref
        .get_with_txn(
          txn,
          &meta_id_from_row_id(row_id, RowMetaKey::IsDocumentEmpty),
        )
        .unwrap_or(true),
      attachment_count: map_ref
        .get_with_txn(
          txn,
          &meta_id_from_row_id(row_id, RowMetaKey::AttachmentCount),
        )
        .unwrap_or(0),
    }
  }

  #[allow(dead_code)]
  pub(crate) fn fill_map_ref(self, txn: &mut TransactionMut, row_id: &Uuid, map_ref: &MapRef) {
    if let Some(icon) = self.icon_url {
      map_ref.try_update(txn, meta_id_from_row_id(row_id, RowMetaKey::IconId), icon);
    }

    if let Some(cover) = self.cover {
      map_ref.try_update(
        txn,
        meta_id_from_row_id(row_id, RowMetaKey::CoverId),
        serde_json::to_string(&cover).unwrap_or_default(),
      );
    }
  }
}
