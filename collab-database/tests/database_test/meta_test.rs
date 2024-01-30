use crate::database_test::helper::DatabaseTestBuilder;

#[tokio::test]
async fn update_database_name_test() {
  let database_test = DatabaseTestBuilder::new(1, "1")
    .with_name("initial_database_name".to_string())
    .build()
    .await;

  let name = database_test.get_database_name();
  assert_eq!(name, "initial_database_name");

  database_test.set_database_name("new_database_name");

  let name = database_test.get_database_name();
  assert_eq!(name, "new_database_name");
}
