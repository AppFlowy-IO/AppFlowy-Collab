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
    id: "r1".to_string(),
    ..Default::default()
  });

  assert_eq!(duplicated_database.rows.get_all_rows().len(), 1);
  assert!(database.rows.get_all_rows().is_empty());
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

  database.create_view(CreateViewParams {
    view_id: "v2".to_string(),
    ..Default::default()
  });

  let duplicated_database = test.duplicate_view("d1", "v").unwrap();
  duplicated_database.push_row(Row {
    id: "r1".to_string(),
    ..Default::default()
  });

  assert_eq!(duplicated_database.rows.get_all_rows().len(), 1);
  assert_eq!(database.rows.get_all_rows().len(), 1);
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
