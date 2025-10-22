use crate::database_test::helper::{DatabaseTest, create_database_with_params};
use collab::database::entity::{CreateDatabaseParams, CreateViewParams};
use collab::database::rows::{CREATED_AT, new_cell_builder};
use collab::database::rows::{CreateRowParams, LAST_MODIFIED};
use collab::util::AnyMapExt;
use uuid::Uuid;

#[tokio::test]
async fn insert_cell_test() {
  let database_id = Uuid::new_v4();
  let row_id = Uuid::new_v4();
  let mut test = user_database_with_default_row(&database_id, row_id).await;
  test
    .update_row(row_id, |row_update| {
      row_update.update_cells(|cells_update| {
        cells_update.insert_cell("f1", {
          let mut cell = new_cell_builder(1);
          cell.insert("level".into(), 1.into());
          cell
        });
      });
    })
    .await;

  let row = test.get_row(&row_id).await.unwrap();
  let cell = row.cells.get("f1").unwrap();
  assert_eq!(cell.get_as::<i64>("level").unwrap(), 1);
}

#[tokio::test]
async fn update_cell_test() {
  let database_id = Uuid::new_v4();
  let row_id = Uuid::new_v4();
  let mut test = user_database_with_default_row(&database_id, row_id).await;
  test
    .update_row(row_id, |row_update| {
      row_update.update_cells(|cells_update| {
        cells_update.insert_cell("f1", {
          let mut cell = new_cell_builder(1);
          cell.insert("level".into(), 1.into());
          cell
        });
      });
    })
    .await;

  test
    .update_row(row_id, |row_update| {
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

  let row = test.get_row(&row_id).await.unwrap();
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
  let database_id = Uuid::new_v4();
  let non_existent_row_id = Uuid::new_v4();
  let mut database = create_database_with_params(CreateDatabaseParams {
    database_id,
    views: vec![CreateViewParams {
      database_id,
      view_id: Uuid::new_v5(&Uuid::NAMESPACE_OID, b"v1"),
      ..Default::default()
    }],
    ..Default::default()
  })
  .await;

  database
    .update_row(non_existent_row_id, |_row_update| {})
    .await;
  let row = database.get_row(&non_existent_row_id).await.unwrap();
  // If the row with the given id does not exist, the get_row method will return a empty Row
  assert!(row.is_empty())
}

async fn user_database_with_default_row(database_id: &Uuid, row_id: Uuid) -> DatabaseTest {
  let mut database = create_database_with_params(CreateDatabaseParams {
    database_id: *database_id,
    views: vec![CreateViewParams {
      database_id: *database_id,
      view_id: Uuid::new_v5(&Uuid::NAMESPACE_OID, b"v1"),
      ..Default::default()
    }],
    ..Default::default()
  })
  .await;

  database
    .create_row_in_view("v1", CreateRowParams::new(row_id, *database_id))
    .await
    .unwrap();

  database
}
