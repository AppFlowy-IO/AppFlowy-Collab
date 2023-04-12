use crate::database::Database;
use crate::fields::Field;
use crate::rows::Row;
use crate::views::DatabaseView;
use serde::Serialize;

#[derive(Serialize)]
pub struct DatabaseSerde {
  pub views: Vec<DatabaseView>,
  pub rows: Vec<Row>,
  pub fields: Vec<Field>,
}

impl DatabaseSerde {
  pub fn from_database(database: &Database) -> DatabaseSerde {
    let txn = database.root.transact();
    let views = database.views.get_all_views_with_txn(&txn);
    let fields = database.fields.get_all_fields_with_txn(&txn);
    let rows = database.get_database_rows_with_txn(&txn);
    Self {
      views,
      rows,
      fields,
    }
  }
}
