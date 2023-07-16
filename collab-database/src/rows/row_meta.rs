use collab::preclude::{MapRef, MapRefExtension, ReadTxn, TransactionMut};
use uuid::Uuid;

use crate::rows::{meta_id_from_row_id, RowMetaKey};

pub struct RowMetaMap<'a>(pub &'a MapRef);

impl<'a> RowMetaMap<'a> {
  pub fn new(map_ref: &'a MapRef) -> Self {
    Self(map_ref)
  }
}

pub struct RowMetaUpdate<'a, 'b, 'c> {
  #[allow(dead_code)]
  map_ref: &'c MapRef,

  #[allow(dead_code)]
  txn: &'a mut TransactionMut<'b>,

  row_id: Uuid,
}

impl<'a, 'b, 'c> RowMetaUpdate<'a, 'b, 'c> {
  pub fn new(txn: &'a mut TransactionMut<'b>, map_ref: &'c MapRef, row_id: Uuid) -> Self {
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

  pub fn insert_icon(self, icon_url: &str) -> Self {
    let icon_id = meta_id_from_row_id(&self.row_id, RowMetaKey::IconId);
    self
      .map_ref
      .insert_str_with_txn(self.txn, &icon_id, icon_url);
    self
  }

  pub fn insert_cover(self, cover_url: &str) -> Self {
    let cover_id = meta_id_from_row_id(&self.row_id, RowMetaKey::CoverId);
    self
      .map_ref
      .insert_str_with_txn(self.txn, &cover_id, cover_url);
    self
  }
}

#[derive(Debug, Clone)]
pub struct RowMeta {
  pub row_id: String,
  pub document_id: String,
  pub icon_url: Option<String>,
  pub cover_url: Option<String>,
}

impl RowMeta {
  pub(crate) fn empty(row_id: Uuid) -> Self {
    Self {
      row_id: row_id.to_string(),
      document_id: meta_id_from_row_id(&row_id, RowMetaKey::DocumentId),
      icon_url: None,
      cover_url: None,
    }
  }

  pub(crate) fn from_map_ref<T: ReadTxn>(txn: &T, row_id: &Uuid, map_ref: &MapRef) -> Self {
    Self {
      row_id: row_id.to_string(),
      document_id: meta_id_from_row_id(row_id, RowMetaKey::DocumentId),
      icon_url: map_ref.get_str_with_txn(txn, &meta_id_from_row_id(row_id, RowMetaKey::IconId)),
      cover_url: map_ref.get_str_with_txn(txn, &meta_id_from_row_id(row_id, RowMetaKey::CoverId)),
    }
  }
}
