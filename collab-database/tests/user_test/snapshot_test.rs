use collab_database::rows::CreateRowParams;
use collab_database::views::CreateDatabaseParams;
use collab_plugins::disk::rocksdb::CollabPersistenceConfig;

use crate::user_test::helper::{user_database_test, user_database_test_with_config};

#[tokio::test]
async fn create_database_row_snapshot_test() {
  let test = user_database_test_with_config(
    1,
    CollabPersistenceConfig::new()
      .enable_snapshot(true)
      .snapshot_per_update(5),
  );
  let database = test
    .create_database(CreateDatabaseParams {
      database_id: "d1".to_string(),
      view_id: "v1".to_string(),
      ..Default::default()
    })
    .unwrap();

  let snapshots = test.get_database_snapshots("d1");
  assert!(snapshots.is_empty());

  for i in 0..10 {
    database
      .create_row(CreateRowParams {
        id: i.into(),
        ..Default::default()
      })
      .unwrap();
  }

  let snapshots = test.get_database_snapshots("d1");
  assert_eq!(snapshots.len(), 2);
}

#[tokio::test]
async fn delete_database_snapshot_test() {
  let test = user_database_test(1);
  let database = test
    .create_database(CreateDatabaseParams {
      database_id: "d1".to_string(),
      view_id: "v1".to_string(),
      ..Default::default()
    })
    .unwrap();

  for i in 0..10 {
    database
      .create_row(CreateRowParams {
        id: i.into(),
        ..Default::default()
      })
      .unwrap();
  }
  test.delete_database("d1");
  let snapshots = test.get_database_snapshots("d1");
  assert!(snapshots.is_empty());
}
