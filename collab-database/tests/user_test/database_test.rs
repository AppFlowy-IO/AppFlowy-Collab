use crate::helper::user_database_test;
use collab_database::rows::Row;
use collab_database::views::{CreateDatabaseParams, CreateViewParams};

#[test]
fn create_multiple_database_test() {
  let test = user_database_test(1);
  test
    .create_database(
      "d1",
      CreateDatabaseParams {
        view_id: "v1".to_string(),
        ..Default::default()
      },
    )
    .unwrap();
  test
    .create_database(
      "d2",
      CreateDatabaseParams {
        view_id: "v1".to_string(),
        ..Default::default()
      },
    )
    .unwrap();
  let all_databases = test.get_all_databases();
  assert_eq!(all_databases.len(), 2);
  assert_eq!(all_databases[0].database_id, "d1");
  assert_eq!(all_databases[1].database_id, "d2");
}

#[test]
fn delete_database_test() {
  let test = user_database_test(1);
  test
    .create_database(
      "d1",
      CreateDatabaseParams {
        view_id: "v1".to_string(),
        ..Default::default()
      },
    )
    .unwrap();
  test
    .create_database(
      "d2",
      CreateDatabaseParams {
        view_id: "v1".to_string(),
        ..Default::default()
      },
    )
    .unwrap();
  test.delete_database("d1");

  let all_databases = test.get_all_databases();
  assert_eq!(all_databases[0].database_id, "d2");
}

#[test]
fn duplicate_database_inline_view_test() {
  let test = user_database_test(1);
  let database = test
    .create_database(
      "d1",
      CreateDatabaseParams {
        view_id: "v1".to_string(),
        ..Default::default()
      },
    )
    .unwrap();

  let duplicated_database = test.duplicate_view("d1", "v1").unwrap();
  duplicated_database.push_row(Row {
    id: 1.into(),
    ..Default::default()
  });

  assert_eq!(duplicated_database.get_rows_for_view("v1").len(), 1);
  assert!(database.get_rows_for_view("v1").is_empty());
}

#[test]
fn duplicate_database_view_test() {
  let test = user_database_test(1);
  let database = test
    .create_database(
      "d1",
      CreateDatabaseParams {
        view_id: "v1".to_string(),
        ..Default::default()
      },
    )
    .unwrap();

  test.create_database_view(CreateViewParams {
    database_id: "d1".to_string(),
    view_id: "v2".to_string(),
    ..Default::default()
  });

  let duplicated_database = test.duplicate_view("d1", "v2").unwrap();
  duplicated_database.push_row(Row {
    id: 1.into(),
    ..Default::default()
  });

  // Duplicated database should have the same rows as the original database
  assert_eq!(duplicated_database.get_rows_for_view("v2").len(), 1);
  assert_eq!(database.get_rows_for_view("v1").len(), 1);
}

#[test]
fn delete_database_inline_view_test() {
  let test = user_database_test(1);
  let database = test
    .create_database(
      "d1",
      CreateDatabaseParams {
        view_id: "v1".to_string(),
        ..Default::default()
      },
    )
    .unwrap();

  for i in 2..5 {
    database.create_view(CreateViewParams {
      view_id: format!("v{}", i),
      ..Default::default()
    });
  }

  let views = database.views.get_all_views();
  assert_eq!(views.len(), 4);

  test.delete_view("d1", "v1");
  let views = database.views.get_all_views();
  assert_eq!(views.len(), 0);
}

#[test]
fn get_database_by_view_id_test() {
  let test = user_database_test(1);
  let _database = test
    .create_database(
      "d1",
      CreateDatabaseParams {
        database_id: "d1".to_string(),
        view_id: "v1".to_string(),
        ..Default::default()
      },
    )
    .unwrap();

  test.create_database_view(CreateViewParams {
    database_id: "d1".to_string(),
    view_id: "v2".to_string(),
    ..Default::default()
  });

  let database = test.get_database_with_view_id("v2");
  assert!(database.is_some());
}
