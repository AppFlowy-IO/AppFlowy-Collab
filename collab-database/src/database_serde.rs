use crate::database::Database;
use crate::fields::Field;
use crate::rows::Row;
use crate::views::View;
use serde::Serialize;

#[derive(Serialize)]
pub struct DatabaseSerde {
  pub rows: Vec<Row>,
  pub views: Vec<View>,
  pub fields: Vec<Field>,
}

impl DatabaseSerde {
  pub fn from_database(database: &Database) -> DatabaseSerde {
    let txn = database.root.transact();
    let rows = database.rows.get_all_rows_with_txn(&txn);
    let views = database.views.get_all_views_with_txn(&txn);
    let fields = database.fields.get_all_fields_with_txn(&txn);

    Self {
      rows,
      views,
      fields,
    }
  }
}
