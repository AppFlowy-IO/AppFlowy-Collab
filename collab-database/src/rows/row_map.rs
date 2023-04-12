use crate::rows::RowComment;
use collab::preclude::{
  Array, ArrayRef, MapRef, MapRefExtension, MapRefWrapper, ReadTxn, TransactionMut, YrsValue,
};

const ROW_META: &str = "row_meta";
const ROW_DOC: &str = "row_doc";
const ROW_COMMENT: &str = "row_comment";

#[derive(Clone)]
pub struct RowMetaMap {
  meta: MapRef,
}

impl RowMetaMap {
  pub fn new_with_txn(txn: &mut TransactionMut, container: &MapRefWrapper) -> Self {
    let meta = container.get_or_insert_map_with_txn(txn, ROW_META);
    meta.get_or_insert_map_with_txn(txn, ROW_DOC);
    meta.get_or_insert_array_with_txn::<RowComment>(txn, ROW_COMMENT);
    Self { meta }
  }

  pub fn get_comments_for_row_with_txn<T: ReadTxn>(&self, txn: &T) -> Vec<RowComment> {
    let array_ref = self.get_comment_array_with_txn(txn);
    array_ref
      .iter(txn)
      .flat_map(|v| {
        if let YrsValue::Any(any) = v {
          RowComment::try_from(any).ok()
        } else {
          None
        }
      })
      .collect()
  }

  pub fn add_comment_with_txn(&self, txn: &mut TransactionMut, comment: RowComment) {
    let array_ref = self.get_comment_array_with_txn(txn);
    array_ref.push_back(txn, comment);
  }

  pub fn remove_comment_with_txn(&self, txn: &mut TransactionMut, index: u32) {
    let array_ref = self.get_comment_array_with_txn(txn);
    array_ref.remove(txn, index);
  }

  #[allow(dead_code)]
  fn get_doc_with_txn<T: ReadTxn>(&self, txn: &T) -> MapRef {
    // It's safe to unwrap because the doc will be inserted when this row gets initialized
    self.meta.get_map_with_txn(txn, ROW_DOC).unwrap()
  }

  fn get_comment_array_with_txn<T: ReadTxn>(&self, txn: &T) -> ArrayRef {
    // It's safe to unwrap because the doc will be inserted when this row gets initialized
    self.meta.get_array_ref_with_txn(txn, ROW_COMMENT).unwrap()
  }
}
