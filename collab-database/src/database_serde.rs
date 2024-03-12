use serde::Serialize;

use crate::database::Database;
use crate::fields::Field;
use crate::rows::Row;
use crate::views::DatabaseView;

#[derive(Serialize)]
pub struct DatabaseSerde {
  pub views: Vec<DatabaseView>,
  pub rows: Vec<Row>,
  pub fields: Vec<Field>,
  pub inline_view: Option<String>,
}

impl DatabaseSerde {
  pub async fn from_database(database: &Database) -> DatabaseSerde {
    let txn = database.root.transact();
    let inline_view = database.metas.get_inline_view_with_txn(&txn);
    let views = database.views.get_all_views_with_txn(&txn);
    let fields = match &inline_view {
      None => vec![],
      Some(view_id) => database.get_fields_in_view_with_txn(&txn, view_id, None),
    };

    let row_orders = match &inline_view {
      None => vec![],
      Some(view_id) => database.views.get_row_orders_with_txn(&txn, view_id),
    };
    drop(txn);
    let rows = database.get_rows_from_row_orders(&row_orders).await;
    Self {
      views,
      rows,
      fields,
      inline_view,
    }
  }
}
