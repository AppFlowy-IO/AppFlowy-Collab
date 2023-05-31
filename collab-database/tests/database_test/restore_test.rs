use std::sync::Arc;

use collab_database::rows::CreateRowParams;
use collab_persistence::kv::rocks_kv::RocksCollabDB;
use serde_json::{json, Value};

use assert_json_diff::assert_json_include;

use crate::database_test::helper::{
  create_database_with_db, restore_database_from_db, DatabaseTest,
};

#[test]
fn restore_row_from_disk_test() {
  let (db, database_test) = create_database_with_db(1, "1");
  let row_1 = CreateRowParams {
    id: 1.into(),
    ..Default::default()
  };
  let row_2 = CreateRowParams {
    id: 2.into(),
    ..Default::default()
  };
  database_test.create_row(row_1.clone());
  database_test.create_row(row_2.clone());
  drop(database_test);

  let database_test = restore_database_from_db(1, "1", db);
  let rows = database_test.get_rows_for_view("v1");
  assert_eq!(rows.len(), 2);

  assert!(rows.iter().any(|row| row.id == row_1.id));
  assert!(rows.iter().any(|row| row.id == row_2.id));
}

#[test]
fn restore_from_disk_test() {
  let (db, database_test, expected) = create_database_with_view();
  assert_json_include!(actual:database_test.to_json_value(), expected:expected);
  // assert_json_eq!(expected, database_test.to_json_value());

  // Restore from disk
  let database_test = restore_database_from_db(1, "1", db);
  assert_json_include!(actual:database_test.to_json_value(), expected:expected);
}

#[test]
fn restore_from_disk_with_different_database_id_test() {
  let (db, _, _) = create_database_with_view();
  let database_test = restore_database_from_db(1, "1", db);
  assert_json_include!(
    expected: json!( {
      "fields": [],
      "inline_view": "v1",
      "rows": [],
      "views": [
        {
          "database_id": "1",
          "field_orders": [],
          "filters": [],
          "group_settings": [],
          "id": "v1",
          "layout": 0,
          "layout_settings": {},
          "name": "my first grid",
          "row_orders": [],
          "sorts": []
        }
      ]
    }),
    actual: database_test.to_json_value()
  );
}

#[test]
fn restore_from_disk_with_different_uid_test() {
  let (db, _, _) = create_database_with_view();
  let database_test = restore_database_from_db(1, "1", db);
  assert_json_include!(
    expected: json!( {
      "fields": [],
      "inline_view": "v1",
      "rows": [],
      "views": [
        {
          "database_id": "1",
          "field_orders": [],
          "filters": [],
          "group_settings": [],
          "id": "v1",
          "layout": 0,
          "layout_settings": {},
          "name": "my first grid",
          "row_orders": [],
          "sorts": []
        }
      ]
    }),
    actual: database_test.to_json_value()
  );
}

fn create_database_with_view() -> (Arc<RocksCollabDB>, DatabaseTest, Value) {
  let (db, database_test) = create_database_with_db(1, "1");
  let expected = json!({
    "fields": [],
    "inline_view": "v1",
    "rows": [],
    "views": [
      {
        "database_id": "1",
        "field_orders": [],
        "filters": [],
        "group_settings": [],
        "id": "v1",
        "layout": 0,
        "layout_settings": {},
        "name": "my first grid",
        "row_orders": [],
        "sorts": []
      }
    ]
  });
  (db, database_test, expected)
}
