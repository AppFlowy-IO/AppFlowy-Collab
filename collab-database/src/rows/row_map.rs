use crate::rows::{row_from_map_ref, Row, RowBuilder, RowUpdate};
use collab::preclude::{MapRefWrapper, ReadTxn, TransactionMut};

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
          .set_cells(row.cells);
      })
      .done();
  }

  pub fn get_row_with_txn<T: ReadTxn>(&self, txn: &T, row_id: &str) -> Option<Row> {
    let map_ref = self.container.get_map_with_txn(txn, row_id)?;
    row_from_map_ref(&map_ref.into_inner(), txn)
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
