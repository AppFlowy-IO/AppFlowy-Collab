use crate::helper::create_user_database;
use collab_database::rows::Row;
use collab_database::views::{CreateDatabaseParams, CreateViewParams};

#[test]
fn create_multiple_database_test() {
  let user_db = create_user_database(1);
  user_db
    .create_database(
      "d1",
      CreateDatabaseParams {
        view_id: "v1".to_string(),
        ..Default::default()
      },
    )
    .unwrap();
  user_db
    .create_database(
      "d2",
      CreateDatabaseParams {
        view_id: "v1".to_string(),
        ..Default::default()
      },
    )
    .unwrap();
  let all_databases = user_db.get_all_databases();
  assert_eq!(all_databases.len(), 2);
  assert_eq!(all_databases[0].database_id, "d1");
  assert_eq!(all_databases[1].database_id, "d2");
}

#[test]
fn delete_database_test() {
  let user_db = create_user_database(1);
  user_db
    .create_database(
      "d1",
      CreateDatabaseParams {
        view_id: "v1".to_string(),
        ..Default::default()
      },
    )
    .unwrap();
  user_db
    .create_database(
      "d2",
      CreateDatabaseParams {
        view_id: "v1".to_string(),
        ..Default::default()
      },
    )
    .unwrap();
  user_db.delete_database("d1");

  let all_databases = user_db.get_all_databases();
  assert_eq!(all_databases[0].database_id, "d2");
}

#[test]
fn duplicate_database_inline_view_test() {
  let user_db = create_user_database(1);
  let database = user_db
    .create_database(
      "d1",
      CreateDatabaseParams {
        view_id: "v1".to_string(),
        ..Default::default()
      },
    )
    .unwrap();

  let duplicated_database = user_db.duplicate_view("d1", "v1").unwrap();
  duplicated_database.push_row(Row {
    id: "r1".to_string(),
    ..Default::default()
  });

  assert_eq!(duplicated_database.rows.get_all_rows().len(), 1);
  assert!(database.rows.get_all_rows().is_empty());
}

#[test]
fn duplicate_database_view_test() {
  let user_db = create_user_database(1);
  let database = user_db
    .create_database(
      "d1",
      CreateDatabaseParams {
        view_id: "v1".to_string(),
        ..Default::default()
      },
    )
    .unwrap();

  database.create_view(CreateViewParams {
    view_id: "v2".to_string(),
    ..Default::default()
  });

  let duplicated_database = user_db.duplicate_view("d1", "v").unwrap();
  duplicated_database.push_row(Row {
    id: "r1".to_string(),
    ..Default::default()
  });

  assert_eq!(duplicated_database.rows.get_all_rows().len(), 1);
  assert_eq!(database.rows.get_all_rows().len(), 1);
}
