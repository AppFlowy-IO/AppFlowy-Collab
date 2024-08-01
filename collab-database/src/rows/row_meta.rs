use collab::preclude::{Map, MapExt, MapRef, ReadTxn, TransactionMut};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::rows::{meta_id_from_row_id, RowMetaKey};

pub struct RowMetaUpdate<'a, 'b> {
  #[allow(dead_code)]
  map_ref: MapRef,

  #[allow(dead_code)]
  txn: &'a mut TransactionMut<'b>,

  row_id: Uuid,
}

impl<'a, 'b, 'c> RowMetaUpdate<'a, 'b> {
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

  pub fn insert_cover_if_not_none(self, cover_url: Option<String>) -> Self {
    if let Some(cover) = cover_url {
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

  pub fn insert_icon(self, icon_url: &str) -> Self {
    let icon_id = meta_id_from_row_id(&self.row_id, RowMetaKey::IconId);
    self.map_ref.insert(self.txn, icon_id, icon_url);
    self
  }

  pub fn insert_cover(self, cover_url: &str) -> Self {
    let cover_id = meta_id_from_row_id(&self.row_id, RowMetaKey::CoverId);
    self.map_ref.insert(self.txn, cover_id, cover_url);
    self
  }

  pub fn update_is_document_empty(self, is_document_empty: bool) -> Self {
    let is_document_empty_id = meta_id_from_row_id(&self.row_id, RowMetaKey::IsDocumentEmpty);
    self
      .map_ref
      .insert(self.txn, is_document_empty_id, is_document_empty);
    self
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RowMeta {
  pub icon_url: Option<String>,
  pub cover_url: Option<String>,
  pub is_document_empty: bool,
}

impl RowMeta {
  pub(crate) fn empty() -> Self {
    Self {
      icon_url: None,
      cover_url: None,
      is_document_empty: true,
    }
  }

  pub(crate) fn from_map_ref<T: ReadTxn>(txn: &T, row_id: &Uuid, map_ref: &MapRef) -> Self {
    Self {
      icon_url: map_ref.get_with_txn(txn, &meta_id_from_row_id(row_id, RowMetaKey::IconId)),
      cover_url: map_ref.get_with_txn(txn, &meta_id_from_row_id(row_id, RowMetaKey::CoverId)),
      is_document_empty: map_ref
        .get_with_txn(
          txn,
          &meta_id_from_row_id(row_id, RowMetaKey::IsDocumentEmpty),
        )
        .unwrap_or(true),
    }
  }

  pub(crate) fn fill_map_ref(self, txn: &mut TransactionMut, row_id: &Uuid, map_ref: &MapRef) {
    if let Some(icon) = self.icon_url {
      map_ref.try_update(txn, meta_id_from_row_id(row_id, RowMetaKey::IconId), icon);
    }

    if let Some(cover) = self.cover_url {
      map_ref.try_update(txn, meta_id_from_row_id(row_id, RowMetaKey::CoverId), cover);
    }
  }
}
