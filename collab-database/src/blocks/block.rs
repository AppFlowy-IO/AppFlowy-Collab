use std::num::NonZeroUsize;
use std::rc::Rc;
use std::sync::Arc;

use collab::plugin_impl::rocks_disk::RocksDiskPlugin;
use collab::preclude::{Collab, CollabBuilder};
use collab_persistence::kv::rocks_kv::RocksCollabDB;
use parking_lot::Mutex;

use lru::LruCache;

use crate::rows::{Cell, CreateRowParams, Row, RowDoc, RowId, RowUpdate};
use crate::views::RowOrder;

#[derive(Clone)]
pub struct Block {
  uid: i64,
  db: Arc<RocksCollabDB>,
  pub cache: Rc<Mutex<LruCache<RowId, Arc<RowDoc>>>>,
}

impl Block {
  pub fn new(uid: i64, db: Arc<RocksCollabDB>) -> Block {
    let cache = Rc::new(Mutex::new(LruCache::new(NonZeroUsize::new(1000).unwrap())));

    Self { uid, db, cache }
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
    let row_order = RowOrder {
      id: row.id,
      height: row.height,
    };
    let row_id = row.id;
    let row_doc = RowDoc::create(row, self.uid, row_id, self.db.clone());
    self.cache.lock().put(row_id, Arc::new(row_doc));
    row_order
  }

  pub fn get_row<R: Into<RowId>>(&self, row_id: R) -> Option<Row> {
    let row_id = row_id.into();
    self.get_or_init_row(row_id).get_row()
  }

  pub fn get_rows_from_row_orders(&self, row_orders: &[RowOrder]) -> Vec<Row> {
    let mut rows = Vec::new();
    for row_order in row_orders {
      let row = self.get_or_init_row(row_order.id).get_row();
      if let Some(row) = row {
        rows.push(row);
      }
    }
    rows
  }

  pub fn get_cell<R: Into<RowId>>(&self, row_id: R, field_id: &str) -> Option<Cell> {
    let row_id = row_id.into();
    self.get_or_init_row(row_id).get_cell(field_id)
  }

  pub fn delete_row(&self, row_id: &RowId) {
    let doc = self.cache.lock().pop(row_id);
    if let Some(doc) = doc {
      doc.delete();
    }
  }

  pub fn update_row<F, R: Into<RowId>>(&self, row_id: R, f: F)
  where
    F: FnOnce(RowUpdate),
  {
    let row = self.cache.lock().get(&row_id.into()).cloned();
    if let Some(row) = row {
      row.update::<F, R>(f);
    }
  }

  fn get_or_init_row(&self, row_id: RowId) -> Arc<RowDoc> {
    let row = self.cache.lock().get(&row_id).cloned();
    if row.is_none() {
      let row = Arc::new(RowDoc::new(self.uid, row_id, self.db.clone()));
      self.cache.lock().put(row_id, row.clone());
      row
    } else {
      row.unwrap()
    }
  }
}
