use collab_plugins::local_storage::CollabPersistenceConfig;

use crate::user_test::async_test::script::{
  create_database, database_test, expected_fields, expected_rows, expected_view, DatabaseScript::*,
};

#[tokio::test]
async fn flush_doc_test() {
  let mut test = database_test(CollabPersistenceConfig::new()).await;
  test
    .run_scripts(vec![
      CreateDatabase {
        params: create_database("d1"),
      },
      CloseDatabase {
        database_id: "d1".to_string(),
      },
      AssertDatabase {
        database_id: "d1".to_string(),
        expected_fields: expected_fields(),
        expected_rows: expected_rows(),
        expected_view: expected_view(),
      },
    ])
    .await;

  test
    .run_scripts(vec![
      OpenDatabase {
        database_id: "d1".to_string(),
      },
      AssertDatabase {
        database_id: "d1".to_string(),
        expected_fields: expected_fields(),
        expected_rows: expected_rows(),
        expected_view: expected_view(),
      },
    ])
    .await;
}
