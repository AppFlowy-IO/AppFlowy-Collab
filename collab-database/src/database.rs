use crate::database_serde::DatabaseSerde;
use crate::error::DatabaseError;
use crate::fields::{Field, FieldMap};
use crate::rows::{Row, RowMap};
use crate::views::{CreateViewParams, View, ViewMap};
use collab::preclude::{Collab, JsonValue, MapRefWrapper};
use std::rc::Rc;

pub struct Database {
  inner: Collab,
  pub(crate) root: MapRefWrapper,
  pub rows: Rc<RowMap>,
  pub views: Rc<ViewMap>,
  pub fields: Rc<FieldMap>,
}

const DATABASE_ID: &str = "id";
const DATABASE: &str = "database";
const FIELDS: &str = "fields";
const ROWS: &str = "rows";
const VIEWS: &str = "views";

pub struct DatabaseContext {}

impl Database {
  pub fn create(
    id: &str,
    collab: Collab,
    _context: DatabaseContext,
  ) -> Result<Self, DatabaseError> {
    if id.is_empty() {
      return Err(DatabaseError::InvalidDatabaseID);
    }

    let (database, fields, rows, views) = collab.with_transact_mut(|txn| {
      // { DATABASE: {:} }
      let database = collab
        .get_map_with_txn(txn, vec![DATABASE])
        .unwrap_or_else(|| collab.create_map_with_txn(txn, DATABASE));

      database.insert_with_txn(txn, DATABASE_ID, id);

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

    Ok(Self {
      inner: collab,
      root: database,
      rows: Rc::new(rows),
      views: Rc::new(views),
      fields: Rc::new(fields),
    })
  }

  pub fn get_database_id(&self) -> Option<String> {
    self.root.get_str(DATABASE_ID)
  }

  pub fn insert_row(&self, row: Row) {
    self.root.with_transact_mut(|txn| {
      self.views.update_all_views_with_txn(txn, |update| {
        update.add_row_order(&row);
      });
      self.rows.insert_row_with_txn(txn, row);
    })
  }

  pub fn insert_field(&self, field: Field) {
    self.root.with_transact_mut(|txn| {
      self.views.update_all_views_with_txn(txn, |update| {
        update.add_field_order(&field);
      });
      self.fields.insert_field_with_txn(txn, field);
    })
  }

  pub fn create_view(&self, params: CreateViewParams) {
    self.root.with_transact_mut(|txn| {
      let field_orders = self.fields.get_all_field_orders(txn);
      let row_orders = self.rows.get_all_row_orders_with_txn(txn);
      let view = View {
        id: params.id,
        database_id: params.database_id,
        name: params.name,
        layout: params.layout,
        layout_settings: params.layout_settings,
        filters: params.filters,
        groups: params.groups,
        sorts: params.sorts,
        row_orders,
        field_orders,
      };
      self.views.insert_view_with_txn(txn, view);
    })
  }

  pub fn to_json_value(&self) -> JsonValue {
    let database_serde = DatabaseSerde::from_database(self);
    serde_json::to_value(&database_serde).unwrap()
  }
}
