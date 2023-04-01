use crate::database_serde::DatabaseSerde;
use crate::error::DatabaseError;
use crate::fields::{Field, FieldMap};
use crate::rows::{Row, RowMap};
use crate::views::{CreateViewParams, RowOrder, View, ViewMap};
use collab::preclude::{Collab, JsonValue, MapRefExtension, MapRefWrapper, ReadTxn};
use std::rc::Rc;

pub struct Database {
  #[allow(dead_code)]
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
    let txn = self.root.transact();
    self.root.get_str_with_txn(&txn, DATABASE_ID)
  }

  pub fn get_database_id_with_txn<T: ReadTxn>(&self, txn: &T) -> Option<String> {
    self.root.get_str_with_txn(txn, DATABASE_ID)
  }

  pub fn insert_row(&self, row: Row) {
    self.root.with_transact_mut(|txn| {
      self.views.update_all_views_with_txn(txn, |update| {
        update.add_row_order(&row);
      });
      self.rows.insert_row_with_txn(txn, row);
    })
  }

  pub fn get_rows_for_view(&self, view_id: &str) -> Vec<Row> {
    let txn = self.root.transact();
    let row_orders = self
      .views
      .get_view_with_txn(&txn, view_id)
      .map(|view| view.row_orders)
      .unwrap_or_default();

    self.get_rows_in_order_with_txn(&txn, &row_orders)
  }

  pub fn get_rows_in_order_with_txn<T: ReadTxn>(
    &self,
    txn: &T,
    row_orders: &[RowOrder],
  ) -> Vec<Row> {
    row_orders
      .iter()
      .flat_map(|row_order| self.rows.get_row_with_txn(txn, &row_order.id))
      .collect::<Vec<Row>>()
  }

  pub fn delete_row(&self, row_id: &str) {
    self.root.with_transact_mut(|txn| {
      self.views.update_all_views_with_txn(txn, |update| {
        update.remove_row_order(row_id);
      });
      self.rows.delete_row_with_txn(txn, row_id);
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

  pub fn delete_field(&self, field_id: &str) {
    self.root.with_transact_mut(|txn| {
      self.views.update_all_views_with_txn(txn, |update| {
        update.remove_field_order(field_id);
      });
      self.fields.delete_field_with_txn(txn, field_id);
    })
  }

  pub fn create_view(&self, params: CreateViewParams) {
    self.root.with_transact_mut(|txn| {
      let field_orders = self.fields.get_all_field_orders(txn);
      let row_orders = self.rows.get_all_row_orders_with_txn(txn);
      let timestamp = chrono::Utc::now().timestamp();
      // It's safe to unwrap. Because the database_id must exist
      let database_id = self.get_database_id_with_txn(txn).unwrap();
      let view = View {
        id: params.id,
        database_id,
        name: params.name,
        layout: params.layout,
        layout_settings: params.layout_settings,
        filters: params.filters,
        groups: params.groups,
        sorts: params.sorts,
        row_orders,
        field_orders,
        created_at: timestamp,
        modified_at: timestamp,
      };
      self.views.insert_view_with_txn(txn, view);
    })
  }

  pub fn to_json_value(&self) -> JsonValue {
    let database_serde = DatabaseSerde::from_database(self);
    serde_json::to_value(&database_serde).unwrap()
  }
}
