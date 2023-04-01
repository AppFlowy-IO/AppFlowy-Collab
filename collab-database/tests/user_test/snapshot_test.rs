use crate::helper::create_user_database;

use collab_database::rows::Row;
use collab_database::views::CreateDatabaseParams;

#[test]
fn database_get_snapshot_test() {
  let user_db = create_user_database(1);
  let database = user_db
    .create_database(
      "d1",
      CreateDatabaseParams {
        view_id: "v1".to_string(),
        ..Default::default()
      },
    )
    .unwrap();

  let snapshots = user_db.get_database_snapshots("d1");
  assert!(snapshots.is_empty());

  for i in 0..10 {
    let row_id = format!("r{}", i);
    database.insert_row(Row::new(row_id));
  }

  let snapshots = user_db.get_database_snapshots("d1");
  assert!(snapshots.is_empty());
}

#[test]
fn delete_database_snapshot_test() {
  let user_db = create_user_database(1);
  let database = user_db
    .create_database(
      "d1",
      CreateDatabaseParams {
        view_id: "v1".to_string(),
        ..Default::default()
      },
    )
    .unwrap();

  for i in 0..10 {
    let row_id = format!("r{}", i);
    database.insert_row(Row::new(row_id));
  }
  user_db.delete_database("d1");
  let snapshots = user_db.get_database_snapshots("d1");
  assert!(snapshots.is_empty());
}

#[test]
fn restore_from_database_snapshot_test() {
  let user_db = create_user_database(1);
  let database = user_db
    .create_database(
      "d1",
      CreateDatabaseParams {
        view_id: "v1".to_string(),
        ..Default::default()
      },
    )
    .unwrap();
  for i in 0..4 {
    let row_id = format!("r{}", i);
    database.insert_row(Row {
      id: row_id,
      ..Default::default()
    });
  }

  let mut snapshots = user_db.get_database_snapshots("d1");
  let database2 = user_db
    .restore_database_from_snapshot("d1", snapshots.remove(0))
    .unwrap();

  let rows = database2.get_rows_for_view("v1");
  assert_eq!(rows.len(), 4);
  let view = database2.views.get_view("v1");
  assert!(view.is_some());
}
