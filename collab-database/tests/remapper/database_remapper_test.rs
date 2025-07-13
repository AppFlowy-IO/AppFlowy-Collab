use collab_database::database_remapper::DatabaseCollabRemapper;
use std::collections::HashMap;
use std::fs;

#[tokio::test]
async fn test_remap_database_with_database_id() {
  let test_db_path = "tests/assets/database_remapper/d5db7722-8919-4e9b-ac2d-8d054015dcb2.collab";
  let db_state = fs::read(test_db_path).expect("Failed to read test database file");

  let mut id_mapping: HashMap<String, String> = HashMap::new();
  id_mapping.insert(
    "d5db7722-8919-4e9b-ac2d-8d054015dcb2".to_string(),
    "new_database_id".to_string(),
  );

  let remapper = DatabaseCollabRemapper::new(id_mapping);
  let database_id = "d5db7722-8919-4e9b-ac2d-8d054015dcb2";
  let user_id = "123456";

  let remapped_state = remapper
    .remap_database_collab_state(database_id, user_id, &db_state)
    .await
    .map_err(|e| {
      eprintln!("Failed to remap database collab state: {:?}", e);
      e
    })
    .unwrap();

  assert!(
    !remapped_state.is_empty(),
    "Remapped state should not be empty"
  );

  let remapped_data = remapper
    .collab_bytes_to_database_data(database_id, user_id, &remapped_state)
    .await
    .unwrap();

  assert_eq!(
    remapped_data.database_id, "new_database_id",
    "Database ID should be remapped"
  );

  for view in remapped_data.views {
    assert_eq!(view.id, "new_database_id", "View ID should be remapped");
    assert_eq!(
      view.database_id, "new_database_id",
      "View database ID should be remapped"
    );
  }
}
