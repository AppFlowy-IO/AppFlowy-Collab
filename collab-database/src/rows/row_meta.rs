use collab::preclude::{MapRef, MapRefExtension, ReadTxn, TransactionMut};

pub const DOCUMENT_ID: &str = "document_id";
pub struct RowMetaMap<'a>(pub &'a MapRef);

impl<'a> RowMetaMap<'a> {
  pub fn new(map_ref: &'a MapRef) -> Self {
    Self(map_ref)
  }

  pub fn get_doc_id_with_txn<T: ReadTxn>(&self, txn: &T) -> Option<String> {
    self.0.get_str_with_txn(txn, DOCUMENT_ID)
  }
}

pub struct RowMetaUpdate<'a, 'b, 'c> {
  #[allow(dead_code)]
  map_ref: &'c MapRef,

  #[allow(dead_code)]
  txn: &'a mut TransactionMut<'b>,
}

impl<'a, 'b, 'c> RowMetaUpdate<'a, 'b, 'c> {
  pub fn new(txn: &'a mut TransactionMut<'b>, map_ref: &'c MapRef) -> Self {
    Self { map_ref, txn }
  }

  pub fn insert_doc_id(self, doc_id: &str) -> Self {
    self
      .map_ref
      .insert_str_with_txn(self.txn, DOCUMENT_ID, doc_id);
    self
  }
}

pub struct RowMeta {
  #[allow(dead_code)]
  pub doc_id: Option<String>,
}

impl RowMeta {
  pub(crate) fn from_map_ref<T: ReadTxn>(txn: &T, map_ref: &MapRef) -> Self {
    Self {
      doc_id: map_ref.get_str_with_txn(txn, DOCUMENT_ID),
    }
  }
}
