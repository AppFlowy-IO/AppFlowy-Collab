use crate::rows::{
  row_from_map_ref, row_from_value, row_id_from_value, Row, RowBuilder, RowUpdate,
};
use crate::views::RowOrder;
use collab::preclude::{Map, MapRefExtension, MapRefWrapper, ReadTxn, TransactionMut};

pub struct RowMap {
  container: MapRefWrapper,
}

impl RowMap {
  pub fn new(container: MapRefWrapper) -> Self {
    Self { container }
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

  pub fn get_all_row_orders_with_txn<T: ReadTxn>(&self, txn: &T) -> Vec<RowOrder> {
    let mut ids = self
      .container
      .iter(txn)
      .flat_map(|(_k, v)| row_id_from_value(v, txn))
      .collect::<Vec<(String, i64)>>();
    ids.sort_by(|(_, left), (_, right)| left.cmp(right));
    ids.into_iter().map(|(id, _)| RowOrder::new(id)).collect()
  }

  pub fn delete_row_with_txn(&self, txn: &mut TransactionMut, row_id: &str) {
    self.container.delete_with_txn(txn, row_id)
  }

  pub fn update_row<F>(&self, row_id: &str, f: F) -> Option<Row>
  where
    F: FnOnce(RowUpdate) -> Option<Row>,
  {
    self.container.with_transact_mut(|txn| {
      let map_ref = self.container.get_map_with_txn(txn, row_id)?;
      let update = RowUpdate::new(row_id, txn, &map_ref);
      f(update)
    })
  }
}
