use crate::helper::create_user_database;
use collab_database::rows::Row;
use collab_database::views::CreateViewParams;

#[test]
fn database_get_snapshot_test() {
  let user_db = create_user_database(1);
  let database = user_db
    .create_database(
      "d1",
      CreateViewParams {
        id: "v1".to_string(),
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
      CreateViewParams {
        id: "v1".to_string(),
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
