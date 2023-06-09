use std::num::NonZeroUsize;
use std::rc::Rc;
use std::sync::Arc;

use collab_persistence::kv::rocks_kv::RocksCollabDB;
use lru::LruCache;
use parking_lot::Mutex;

use crate::rows::{Cell, DatabaseRow, Row, RowId, RowMeta, RowUpdate};
use crate::user::DatabaseCollabBuilder;
use crate::views::RowOrder;

/// Each [Block] contains a list of [DatabaseRow]s. Each [DatabaseRow] represents a row in the database.
/// Currently, we only use one [Block] to manage all the rows in the database. In the future, we
/// might want to split the rows into multiple [Block]s to improve performance.
#[derive(Clone)]
pub struct Block {
  uid: i64,
  db: Arc<RocksCollabDB>,
  collab_builder: Arc<dyn DatabaseCollabBuilder>,
  pub cache: Rc<Mutex<LruCache<RowId, Arc<DatabaseRow>>>>,
}

impl Block {
  pub fn new(
    uid: i64,
    db: Arc<RocksCollabDB>,
    collab_builder: Arc<dyn DatabaseCollabBuilder>,
  ) -> Block {
    let cache = Rc::new(Mutex::new(LruCache::new(NonZeroUsize::new(1000).unwrap())));

    Self {
      uid,
      db,
      cache,
      collab_builder,
    }
  }

  pub fn create_rows<T: Into<Row>>(&self, rows: Vec<T>) -> Vec<RowOrder> {
    let mut row_orders: Vec<RowOrder> = vec![];
    for row in rows.into_iter() {
      let row_order = self.create_row(row);
      row_orders.push(row_order);
    }
    row_orders
  }

  pub fn create_row<T: Into<Row>>(&self, row: T) -> RowOrder {
    let row = row.into();
    let row_id = row.id.clone();
    let row_order = RowOrder {
      id: row.id.clone(),
      height: row.height,
    };
    let row_doc = DatabaseRow::create(
      row,
      self.uid,
      row_id.clone(),
      self.db.clone(),
      self.collab_builder.clone(),
    );
    self.cache.lock().put(row_id, Arc::new(row_doc));
    row_order
  }

  pub fn get_row(&self, row_id: &RowId) -> Option<Row> {
    self.get_or_init_row(row_id)?.get_row()
  }

  pub fn get_row_meta(&self, row_id: &RowId) -> Option<RowMeta> {
    Some(self.get_or_init_row(row_id)?.get_row_meta())
  }

  pub fn get_rows_from_row_orders(&self, row_orders: &[RowOrder]) -> Vec<Row> {
    let mut rows = Vec::new();
    for row_order in row_orders {
      if let Some(row) = self
        .get_or_init_row(&row_order.id)
        .and_then(|row| row.get_row())
      {
        rows.push(row);
      }
    }
    rows
  }

  pub fn get_cell(&self, row_id: &RowId, field_id: &str) -> Option<Cell> {
    self.get_or_init_row(row_id)?.get_cell(field_id)
  }

  pub fn delete_row(&self, row_id: &RowId) {
    let doc = self.cache.lock().pop(row_id);
    if let Some(doc) = doc {
      doc.delete();
    }
  }

  pub fn update_row<F>(&self, row_id: &RowId, f: F)
  where
    F: FnOnce(RowUpdate),
  {
    let row = self.cache.lock().get(row_id).cloned();
    if let Some(row) = row {
      row.update::<F>(f);
    }
  }

  /// Get the [DatabaseRow] from the cache. If the row is not in the cache, initialize it.
  fn get_or_init_row(&self, row_id: &RowId) -> Option<Arc<DatabaseRow>> {
    let row = self.cache.lock().get(row_id).cloned();
    match row {
      None => {
        let row = Arc::new(DatabaseRow::new(
          self.uid,
          row_id.clone(),
          self.db.clone(),
          self.collab_builder.clone(),
        ));
        self.cache.lock().put(row_id.clone(), row.clone());
        Some(row)
      },
      Some(row) => Some(row),
    }
  }
}
