use crate::rows::{
  row_from_map_ref, row_from_value, row_id_from_value, row_order_from_value, Row, RowBuilder,
  RowComment, RowUpdate,
};
use crate::views::RowOrder;
use collab::preclude::{
  Array, ArrayRef, Map, MapRef, MapRefExtension, MapRefWrapper, ReadTxn, TransactionMut, YrsValue,
};

const ROW_META: &str = "row_meta";
const ROW_DOC: &str = "row_doc";
const ROW_COMMENT: &str = "row_comment";

pub struct RowMap {
  container: MapRefWrapper,
  meta: MapRef,
}

impl RowMap {
  pub fn new_with_txn(txn: &mut TransactionMut, container: MapRefWrapper) -> Self {
    let meta = container.get_or_insert_map_with_txn(txn, ROW_META);
    meta.get_or_insert_map_with_txn(txn, ROW_DOC);
    meta.get_or_insert_array_with_txn::<RowComment>(txn, ROW_COMMENT);
    Self { container, meta }
  }

  pub fn insert_row(&self, row: Row) {
    self
      .container
      .with_transact_mut(|txn| self.insert_row_with_txn(txn, row))
  }

  pub fn insert_row_with_txn(&self, txn: &mut TransactionMut, row: Row) {
    let map_ref = self.container.insert_map_with_txn(txn, &row.id);
    RowBuilder::new(&row.id, txn, map_ref)
      .update(|update| {
        update
          .set_height(row.height)
          .set_visibility(row.visibility)
          .set_created_at(row.created_at)
          .set_cells(row.cells);
      })
      .done();
  }

  pub fn get_row(&self, row_id: &str) -> Option<Row> {
    let txn = self.container.transact();
    self.get_row_with_txn(&txn, row_id)
  }

  pub fn get_row_with_txn<T: ReadTxn>(&self, txn: &T, row_id: &str) -> Option<Row> {
    let map_ref = self.container.get_map_with_txn(txn, row_id)?;
    row_from_map_ref(&map_ref.into_inner(), txn)
  }

  pub fn get_all_rows(&self) -> Vec<Row> {
    let txn = self.container.transact();
    self.get_all_rows_with_txn(&txn)
  }

  pub fn get_all_rows_with_txn<T: ReadTxn>(&self, txn: &T) -> Vec<Row> {
    self
      .container
      .iter(txn)
      .flat_map(|(_k, v)| row_from_value(v, txn))
      .collect::<Vec<_>>()
  }

  pub fn get_all_row_orders(&self) -> Vec<RowOrder> {
    let txn = self.container.transact();
    self.get_all_row_orders_with_txn(&txn)
  }

  pub fn get_all_row_orders_with_txn<T: ReadTxn>(&self, txn: &T) -> Vec<RowOrder> {
    let mut ids = self
      .container
      .iter(txn)
      .flat_map(|(_k, v)| row_order_from_value(v, txn))
      .collect::<Vec<(RowOrder, i64)>>();
    ids.sort_by(|(_, left), (_, right)| left.cmp(right));
    ids.into_iter().map(|(row_order, _)| row_order).collect()
  }

  pub fn delete_row_with_txn(&self, txn: &mut TransactionMut, row_id: &str) {
    self.container.delete_with_txn(txn, row_id)
  }

  pub fn update_row<F>(&self, row_id: &str, f: F)
  where
    F: FnOnce(RowUpdate),
  {
    self.container.with_transact_mut(|txn| {
      let map_ref = self.container.get_or_insert_map_with_txn(txn, row_id);
      let update = RowUpdate::new(row_id, txn, &map_ref);
      f(update)
    })
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
