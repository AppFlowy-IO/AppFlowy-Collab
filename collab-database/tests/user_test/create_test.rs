use crate::helper::create_user_database;
use collab_database::views::CreateViewParams;

#[test]
fn create_multiple_database_test() {
  let user_db = create_user_database(1);
  user_db
    .create_database(
      "d1",
      CreateViewParams {
        id: "v1".to_string(),
        ..Default::default()
      },
    )
    .unwrap();
  user_db
    .create_database(
      "d2",
      CreateViewParams {
        id: "v1".to_string(),
        ..Default::default()
      },
    )
    .unwrap();
  let all_databases = user_db.get_all_databases();
  assert_eq!(all_databases.len(), 2);
  assert_eq!(all_databases[0].database_id, "d1");
  assert_eq!(all_databases[1].database_id, "d2");
}

#[test]
fn delete_database_test() {
  let user_db = create_user_database(1);
  user_db
    .create_database(
      "d1",
      CreateViewParams {
        id: "v1".to_string(),
        ..Default::default()
      },
    )
    .unwrap();
  user_db
    .create_database(
      "d2",
      CreateViewParams {
        id: "v1".to_string(),
        ..Default::default()
      },
    )
    .unwrap();
  user_db.delete_database("d1");

  let all_databases = user_db.get_all_databases();
  assert_eq!(all_databases[0].database_id, "d2");
}
