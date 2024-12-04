use crate::database_test::helper::create_database_with_default_data;

#[tokio::test]
async fn get_cells_for_field_test() {
  let database_id = uuid::Uuid::new_v4();
  let database_test = create_database_with_default_data(1, &database_id.to_string()).await;
  let _rows = database_test.collect_all_rows().await;

  let cells = database_test.get_cells_for_field("v1", "f1").await;
  assert_eq!(cells.len(), 3);

  let cells = database_test.get_cells_for_field("v1", "f2").await;
  assert_eq!(cells.len(), 3);

  let cells = database_test.get_cells_for_field("v1", "f3").await;
  assert_eq!(cells.len(), 3);
}
