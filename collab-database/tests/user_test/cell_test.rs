use collab::core::any_map::AnyMapExtension;
use collab_database::rows::{new_cell_builder, CREATED_AT};
use collab_database::rows::{CreateRowParams, LAST_MODIFIED};
use collab_database::views::{CreateDatabaseParams, CreateViewParams};

use crate::user_test::helper::{workspace_database_test, WorkspaceDatabaseTest};

#[tokio::test]
async fn insert_cell_test() {
  let test = user_database_with_default_row().await;
  let database = test.get_database("d1").await.unwrap();
  database.lock().update_row(&1.into(), |row_update| {
    row_update.update_cells(|cells_update| {
      cells_update.insert_cell(
        "f1",
        new_cell_builder(1).insert_i64_value("level", 1).build(),
      );
    });
  });

  let row = database.lock().get_row(&1.into());
  let cell = row.cells.get("f1").unwrap();
  assert_eq!(cell.get_i64_value("level").unwrap(), 1);
}

#[tokio::test]
async fn update_cell_test() {
  let test = user_database_with_default_row().await;
  let database = test.get_database("d1").await.unwrap();
  database.lock().update_row(&1.into(), |row_update| {
    row_update.update_cells(|cells_update| {
      cells_update.insert_cell(
        "f1",
        new_cell_builder(1).insert_i64_value("level", 1).build(),
      );
    });
  });

  database.lock().update_row(&1.into(), |row_update| {
    row_update.update_cells(|cells_update| {
      cells_update.insert(
        "f1",
        new_cell_builder(1)
          .insert_i64_value("level", 2)
          .insert_str_value("name", "appflowy")
          .build(),
      );
    });
  });

  let row = database.lock().get_row(&1.into());
  let cell = row.cells.get("f1").unwrap();
  let created_at = cell.get_i64_value(CREATED_AT).unwrap();
  let modified_at = cell.get_i64_value(LAST_MODIFIED).unwrap();
  assert!(created_at > 0);
  assert!(modified_at > 0);
  assert_eq!(cell.get_i64_value("level").unwrap(), 2);
  assert_eq!(cell.get_str_value("name").unwrap(), "appflowy");
}

#[tokio::test]
async fn update_not_exist_row_test() {
  let test = workspace_database_test(1).await;
  let database = test
    .create_database(CreateDatabaseParams {
      database_id: "d1".to_string(),
      inline_view_id: "inline_view_id".to_string(),
      views: vec![CreateViewParams {
        database_id: "d1".to_string(),
        view_id: "v1".to_string(),
        ..Default::default()
      }],
      ..Default::default()
    })
    .unwrap();

  database.lock().update_row(&1.into(), |_row_update| {});
  let row = database.lock().get_row(&1.into());
  // If the row with the given id does not exist, the get_row method will return a empty Row
  assert!(row.is_empty())
}

async fn user_database_with_default_row() -> WorkspaceDatabaseTest {
  let test = workspace_database_test(1).await;
  let database = test
    .create_database(CreateDatabaseParams {
      database_id: "d1".to_string(),
      inline_view_id: "inline_view_id".to_string(),
      views: vec![CreateViewParams {
        database_id: "d1".to_string(),
        view_id: "v1".to_string(),
        ..Default::default()
      }],
      ..Default::default()
    })
    .unwrap();

  database.lock().create_row_in_view(
    "v1",
    CreateRowParams {
      id: 1.into(),
      ..Default::default()
    },
  );

  test
}
