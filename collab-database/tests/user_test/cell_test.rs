use collab::util::AnyMapExt;
use collab_database::entity::{CreateDatabaseParams, CreateViewParams};
use collab_database::rows::{CREATED_AT, RowId, new_cell_builder};
use collab_database::rows::{CreateRowParams, LAST_MODIFIED};
use uuid::Uuid;

use crate::user_test::helper::{WorkspaceDatabaseTest, workspace_database_test};

#[tokio::test]
async fn insert_cell_test() {
  let database_id = Uuid::new_v4();
  let row_id = Uuid::new_v4();
  let test = user_database_with_default_row(&database_id, row_id).await;
  let database = test
    .get_or_init_database(&database_id.to_string())
    .await
    .unwrap();
  database
    .write()
    .await
    .update_row(row_id.into(), |row_update| {
      row_update.update_cells(|cells_update| {
        cells_update.insert_cell("f1", {
          let mut cell = new_cell_builder(1);
          cell.insert("level".into(), 1.into());
          cell
        });
      });
    })
    .await;

  let row = database.read().await.get_row(&RowId::from(row_id)).await;
  let cell = row.cells.get("f1").unwrap();
  assert_eq!(cell.get_as::<i64>("level").unwrap(), 1);
}

#[tokio::test]
async fn update_cell_test() {
  let database_id = Uuid::new_v4();
  let row_id = Uuid::new_v4();
  let test = user_database_with_default_row(&database_id, row_id).await;
  let database = test
    .get_or_init_database(&database_id.to_string())
    .await
    .unwrap();
  let mut db = database.write().await;
  db.update_row(row_id.into(), |row_update| {
    row_update.update_cells(|cells_update| {
      cells_update.insert_cell("f1", {
        let mut cell = new_cell_builder(1);
        cell.insert("level".into(), 1.into());
        cell
      });
    });
  })
  .await;

  db.update_row(row_id.into(), |row_update| {
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

  let row = db.get_row(&RowId::from(row_id)).await;
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
  let mut test = workspace_database_test(1).await;
  let database_id = Uuid::new_v4();
  let non_existent_row_id = Uuid::new_v4();
  let database = test
    .create_database(CreateDatabaseParams {
      database_id: database_id.to_string(),
      views: vec![CreateViewParams {
        database_id: database_id.to_string(),
        view_id: "v1".to_string(),
        ..Default::default()
      }],
      ..Default::default()
    })
    .await
    .unwrap();

  let mut db = database.write().await;
  db.update_row(non_existent_row_id.into(), |_row_update| {})
    .await;
  let row = db.get_row(&RowId::from(non_existent_row_id)).await;
  // If the row with the given id does not exist, the get_row method will return a empty Row
  assert!(row.is_empty())
}

async fn user_database_with_default_row(database_id: &Uuid, row_id: Uuid) -> WorkspaceDatabaseTest {
  let mut test = workspace_database_test(1).await;
  let database = test
    .create_database(CreateDatabaseParams {
      database_id: database_id.to_string(),
      views: vec![CreateViewParams {
        database_id: database_id.to_string(),
        view_id: "v1".to_string(),
        ..Default::default()
      }],
      ..Default::default()
    })
    .await
    .unwrap();

  database
    .write()
    .await
    .create_row_in_view("v1", CreateRowParams::new(row_id, database_id.to_string()))
    .await
    .unwrap();

  test
}
