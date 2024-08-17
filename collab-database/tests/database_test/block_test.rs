use collab_database::rows::CreateRowParams;

use crate::database_test::helper::create_database;

#[tokio::test]
async fn create_rows_test() {
  let database_id = uuid::Uuid::new_v4().to_string();
  let mut database_test = create_database(1, &database_id);
  for i in 0..100 {
    database_test.create_row_in_view("v1", CreateRowParams::new(i.to_string(), "1".to_string()));
  }
  let rows = database_test.get_rows_for_view("v1").await;
  assert_eq!(rows.len(), 100);
}
