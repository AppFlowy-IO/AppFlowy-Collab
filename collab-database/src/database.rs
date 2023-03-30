use crate::fields::FieldMap;
use crate::rows::RowMap;
use crate::views::ViewMap;
use collab::preclude::{Collab, MapRefWrapper};
use std::rc::Rc;

pub struct Database {
  inner: Collab,
  database: MapRefWrapper,
  pub rows: Rc<RowMap>,
  pub views: Rc<ViewMap>,
  pub fields: Rc<FieldMap>,
}

const DATABASE: &str = "database";
const FIELDS: &str = "fields";
const ROWS: &str = "rows";
const VIEWS: &str = "views";

pub struct DatabaseContext {}

impl Database {
  pub fn create(collab: Collab, context: DatabaseContext) -> Self {
    let (database, fields, rows, views) = collab.with_transact_mut(|txn| {
      // { DATABASE: {:} }
      let database = collab
        .get_map_with_txn(txn, vec![DATABASE])
        .unwrap_or_else(|| collab.create_map_with_txn(txn, DATABASE));

      // { DATABASE: { FIELDS: {:} } }
      let fields = collab
        .get_map_with_txn(txn, vec![DATABASE, FIELDS])
        .unwrap_or_else(|| database.insert_map_with_txn(txn, FIELDS));

      // { DATABASE: { FIELDS: {:}, ROWS: {:} } }
      let rows = collab
        .get_map_with_txn(txn, vec![DATABASE, ROWS])
        .unwrap_or_else(|| database.insert_map_with_txn(txn, ROWS));

      // { DATABASE: { FIELDS: {:}, ROWS: {:}, VIEWS: {:} } }
      let views = collab
        .get_map_with_txn(txn, vec![DATABASE, VIEWS])
        .unwrap_or_else(|| database.insert_map_with_txn(txn, VIEWS));

      (database, fields, rows, views)
    });
    let rows = RowMap::new(rows);
    let views = ViewMap::new(views);
    let fields = FieldMap::new(fields);

    Self {
      inner: collab,
      database,
      rows: Rc::new(rows),
      views: Rc::new(views),
      fields: Rc::new(fields),
    }
  }
}
