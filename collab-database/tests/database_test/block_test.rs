use collab_database::rows::CreateRowParams;
use uuid::Uuid;

use crate::database_test::helper::create_database;

#[tokio::test]
async fn create_one_row_test() {
  let database_id = Uuid::new_v4().to_string();
  let mut database_test = create_database(1, &database_id);
  for _ in 0..100 {}
  let row_id = Uuid::new_v4();
  database_test
    .create_row_in_view("v1", CreateRowParams::new(row_id, database_id.clone()))
    .await
    .unwrap();
  let rows = database_test.get_rows_for_view("v1").await;
  assert_eq!(rows.len(), 1);
}

#[tokio::test]
async fn create_rows_test() {
  let database_id = Uuid::new_v4().to_string();
  let mut database_test = create_database(1, &database_id);
  for _ in 0..100 {
    let row_id = Uuid::new_v4();
    database_test
      .create_row_in_view("v1", CreateRowParams::new(row_id, database_id.clone()))
      .await
      .unwrap();
  }
  let rows = database_test.get_rows_for_view("v1").await;
  assert_eq!(rows.len(), 100);
}
