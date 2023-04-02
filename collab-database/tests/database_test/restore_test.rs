use crate::helper::{create_database_from_db, create_database_with_db, DatabaseTest};
use assert_json_diff::assert_json_eq;
use collab_database::rows::Row;
use collab_database::views::CreateViewParams;
use collab_persistence::CollabKV;
use serde_json::{json, Value};
use std::sync::Arc;

#[test]
fn restore_row_from_disk_test() {
  let (db, database_test) = create_database_with_db(1, "1");
  let row_1 = Row {
    id: "r1".to_string(),
    ..Default::default()
  };
  let row_2 = Row {
    id: "r2".to_string(),
    ..Default::default()
  };
  database_test.push_row(row_1.clone());
  database_test.push_row(row_2.clone());
  drop(database_test);

  let database_test = create_database_from_db(1, "1", db);
  let rows = database_test.rows.get_all_rows();
  assert_eq!(rows.len(), 2);

  assert!(rows.iter().any(|row| row.id == row_1.id));
  assert!(rows.iter().any(|row| row.id == row_2.id));
}

#[test]
fn restore_from_disk_test() {
  let (db, database_test, expected) = create_database_with_view();
  assert_json_eq!(expected, database_test.to_json_value());

  // Restore from disk
  let database_test = create_database_from_db(1, "1", db);
  assert_json_eq!(expected, database_test.to_json_value());
}

#[test]
fn restore_from_disk_with_different_database_id_test() {
  let (db, _, _) = create_database_with_view();
  let database_test = create_database_from_db(1, "2", db);
  assert_json_eq!(
    json!({
      "fields": [],
      "rows": [],
      "views": []
    }),
    database_test.to_json_value()
  );
}

#[test]
fn restore_from_disk_with_different_uid_test() {
  let (db, _, _) = create_database_with_view();
  let database_test = create_database_from_db(2, "1", db);
  assert_json_eq!(
    json!( {
      "fields": [],
      "rows": [],
      "views": []
    }),
    database_test.to_json_value()
  );
}

fn create_database_with_view() -> (Arc<CollabKV>, DatabaseTest, Value) {
  let (db, database_test) = create_database_with_db(1, "1");
  let params = CreateViewParams {
    view_id: "v1".to_string(),
    name: "my first grid".to_string(),
    ..Default::default()
  };
  database_test.create_view(params);
  let expected = json!({
    "fields": [],
    "rows": [],
    "views": [
      {
        "created_at": 0,
        "database_id": "1",
        "field_orders": [],
        "filters": [],
        "groups": [],
        "id": "v1",
        "layout": 0,
        "layout_settings": {},
        "modified_at": 0,
        "name": "my first grid",
        "row_orders": [],
        "sorts": []
      }
    ]
  });
  (db, database_test, expected)
}
