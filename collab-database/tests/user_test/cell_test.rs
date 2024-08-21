use collab::util::AnyMapExt;
use collab_database::entity::{CreateDatabaseParams, CreateViewParams};
use collab_database::rows::{new_cell_builder, CREATED_AT};
use collab_database::rows::{CreateRowParams, LAST_MODIFIED};

use crate::user_test::helper::{workspace_database_test, WorkspaceDatabaseTest};

#[tokio::test]
async fn insert_cell_test() {
  let test = user_database_with_default_row().await;
  let database = test.get_or_create_database("d1").await.unwrap();
  database
    .write()
    .await
    .update_row(1.into(), |row_update| {
      row_update.update_cells(|cells_update| {
        cells_update.insert_cell("f1", {
          let mut cell = new_cell_builder(1);
          cell.insert("level".into(), 1.into());
          cell
        });
      });
    })
    .await;

  let row = database.read().await.get_row(&1.into()).await;
  let cell = row.cells.get("f1").unwrap();
  assert_eq!(cell.get_as::<i64>("level").unwrap(), 1);
}

#[tokio::test]
async fn update_cell_test() {
  let test = user_database_with_default_row().await;
  let database = test.get_or_create_database("d1").await.unwrap();
  let mut db = database.write().await;
  db.update_row(1.into(), |row_update| {
    row_update.update_cells(|cells_update| {
      cells_update.insert_cell("f1", {
        let mut cell = new_cell_builder(1);
        cell.insert("level".into(), 1.into());
        cell
      });
    });
  })
  .await;

  db.update_row(1.into(), |row_update| {
    row_update.update_cells(|cells_update| {
      cells_update.insert("f1", {
        let mut cell = new_cell_builder(1);
        cell.insert("level".into(), 2.into());
        cell.insert("name".into(), "appflowy".into());
        cell
      });
    });
  })
  .await;

  let row = db.get_row(&1.into()).await;
  let cell = row.cells.get("f1").unwrap();
  let created_at: i64 = cell.get_as(CREATED_AT).unwrap();
  let modified_at: i64 = cell.get_as(LAST_MODIFIED).unwrap();
  assert!(created_at > 0);
  assert!(modified_at > 0);
  assert_eq!(cell.get_as::<i64>("level").unwrap(), 2);
  assert_eq!(
    cell.get_as::<String>("name").unwrap(),
    "appflowy".to_string()
  );
}

#[tokio::test]
async fn update_not_exist_row_test() {
  let mut test = workspace_database_test(1);
  let database = test
    .create_database(CreateDatabaseParams {
      database_id: "d1".to_string(),
      inline_view_id: "v1".to_string(),
      views: vec![CreateViewParams {
        database_id: "d1".to_string(),
        view_id: "v1".to_string(),
        ..Default::default()
      }],
      ..Default::default()
    })
    .unwrap();

  let mut db = database.write().await;
  db.update_row(1.into(), |_row_update| {}).await;
  let row = db.get_row(&1.into()).await;
  // If the row with the given id does not exist, the get_row method will return a empty Row
  assert!(row.is_empty())
}

async fn user_database_with_default_row() -> WorkspaceDatabaseTest {
  let database_id = "d1".to_string();
  let mut test = workspace_database_test(1);
  let database = test
    .create_database(CreateDatabaseParams {
      database_id: database_id.clone(),
      inline_view_id: "v1".to_string(),
      views: vec![CreateViewParams {
        database_id: "d1".to_string(),
        view_id: "v1".to_string(),
        ..Default::default()
      }],
      ..Default::default()
    })
    .unwrap();

  database
    .write()
    .await
    .create_row_in_view("v1", CreateRowParams::new(1, database_id));

  test
}
