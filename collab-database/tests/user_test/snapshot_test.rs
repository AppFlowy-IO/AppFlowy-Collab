use collab::plugin_impl::rocks_disk::Config;
use collab_database::rows::CreateRowParams;
use collab_database::views::CreateDatabaseParams;

use crate::user_test::helper::{user_database_test, user_database_test_with_config};

#[test]
fn create_database_row_snapshot_test() {
  let test = user_database_test_with_config(1, Config::new().snapshot_per_update(5));
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
    database.create_row(CreateRowParams {
      id: i.into(),
      ..Default::default()
    });
  }

  let snapshots = test.get_database_snapshots("d1");
  assert_eq!(snapshots.len(), 2);
}

#[test]
fn delete_database_snapshot_test() {
  let test = user_database_test(1);
  let database = test
    .create_database(CreateDatabaseParams {
      database_id: "d1".to_string(),
      view_id: "v1".to_string(),
      ..Default::default()
    })
    .unwrap();

  for i in 0..10 {
    database.create_row(CreateRowParams {
      id: i.into(),
      ..Default::default()
    });
  }
  test.delete_database("d1");
  let snapshots = test.get_database_snapshots("d1");
  assert!(snapshots.is_empty());
}

#[test]
fn restore_rows_database_snapshot_test() {
  let test = user_database_test_with_config(1, Config::new().snapshot_per_update(5));
  let database = test
    .create_database(CreateDatabaseParams {
      database_id: "d1".to_string(),
      view_id: "v1".to_string(),
      ..Default::default()
    })
    .unwrap();
  for i in 0..5 {
    database.create_row(CreateRowParams {
      id: i.into(),
      ..Default::default()
    });
  }
  let rows = database.get_rows_for_view("v1");
  assert_eq!(rows.len(), 5);

  let mut snapshots = test.get_database_snapshots("d1");
  assert_eq!(snapshots.len(), 1);
  let database2 = test
    .restore_database_from_snapshot("d1", snapshots.remove(0))
    .unwrap();

  let rows = database2.get_rows_for_view("v1");
  assert_eq!(rows.len(), 3);
  let view = database2.views.get_view("v1");
  assert!(view.is_some());
}
