use collab::database::database::DatabaseData;
use collab::database::database_remapper::DatabaseCollabRemapper;
use collab::entity::CollabDocState;
use std::collections::HashMap;
use std::fs;
use uuid::Uuid;

#[tokio::test]
async fn test_remap_database_with_database_id() {
  let test_db_path =
    "tests/database/assets/database_remapper/d5db7722-8919-4e9b-ac2d-8d054015dcb2.collab";
  let db_state: CollabDocState = fs::read(test_db_path)
    .expect("Failed to read test database file")
    .into();

  let mut id_mapping: HashMap<String, String> = HashMap::new();
  // database id
  id_mapping.insert(
    "0640671f-ebb9-4673-a4ac-5d4dbeb011e4".to_string(),
    "00000000-0000-0000-0000-000000000000".to_string(),
  );
  // view ids
  id_mapping.insert(
    "d5db7722-8919-4e9b-ac2d-8d054015dcb2".to_string(),
    "11111111-1111-1111-1111-111111111111".to_string(),
  );
  id_mapping.insert(
    "d65b0117-651f-4d23-b6a4-0eae1fe31f1f".to_string(),
    "11111111-1111-1111-1111-111111111111".to_string(),
  );
  // row ids (from debug output)
  id_mapping.insert(
    "537c08eb-8099-4215-b9dc-8358fa614ee6".to_string(),
    "22222222-2222-2222-2222-222222222222".to_string(),
  );
  id_mapping.insert(
    "3357c1ba-c044-413c-835e-da539b20c01b".to_string(),
    "33333333-3333-3333-3333-333333333333".to_string(),
  );
  id_mapping.insert(
    "f82c4e38-e01e-4b28-a8ea-97c458886440".to_string(),
    "44444444-4444-4444-4444-444444444444".to_string(),
  );
  id_mapping.insert(
    "4f35b03b-e99a-46ab-8a1c-aca4091a8fdd".to_string(),
    "55555555-5555-5555-5555-555555555555".to_string(),
  );
  id_mapping.insert(
    "9d223cb5-af69-4aa7-b0a2-a2ef8d34db11".to_string(),
    "66666666-6666-6666-6666-666666666666".to_string(),
  );

  let remapper = DatabaseCollabRemapper::new(id_mapping);
  let database_id = "0640671f-ebb9-4673-a4ac-5d4dbeb011e4";
  let user_id = "123456";

  let remapped_state = remapper
    .remap_database_collab_state(database_id, user_id, &db_state, None)
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
    .collab_bytes_to_database_data(database_id, user_id, &remapped_state, None)
    .await
    .unwrap();

  assert_eq!(
    remapped_data.database_id.to_string(),
    "00000000-0000-0000-0000-000000000000",
    "Database ID should be remapped"
  );

  for view in &remapped_data.views {
    assert_eq!(
      view.id.to_string(),
      "11111111-1111-1111-1111-111111111111",
      "View ID should be remapped"
    );
    assert_eq!(
      view.database_id.to_string(),
      "00000000-0000-0000-0000-000000000000",
      "View database ID should be remapped"
    );

    // Verify row orders in views are remapped
    for row_order in &view.row_orders {
      let row_id_str = row_order.id.to_string();
      assert!(
        [
          "22222222-2222-2222-2222-222222222222",
          "33333333-3333-3333-3333-333333333333",
          "44444444-4444-4444-4444-444444444444",
          "55555555-5555-5555-5555-555555555555",
          "66666666-6666-6666-6666-666666666666"
        ]
        .contains(&row_id_str.as_str()),
        "Row order ID should be remapped: {}",
        row_id_str
      );
    }
  }

  // Verify actual rows are remapped (if any exist)
  for row in &remapped_data.rows {
    let row_id_str = row.id.to_string();
    assert!(
      [
        "22222222-2222-2222-2222-222222222222",
        "33333333-3333-3333-3333-333333333333",
        "44444444-4444-4444-4444-444444444444",
        "55555555-5555-5555-5555-555555555555",
        "66666666-6666-6666-6666-666666666666"
      ]
      .contains(&row_id_str.as_str()),
      "Row ID should be remapped: {}",
      row_id_str
    );
    assert_eq!(
      row.database_id.to_string(),
      "00000000-0000-0000-0000-000000000000",
      "Row database ID should be remapped"
    );
  }
}

#[tokio::test]
async fn test_database_remapper_with_row_meta() {
  let test_json_path = "tests/database/assets/row_meta/50dfa70a-d53c-4b7d-8b49-6194aadbac2a.json";
  let json_content = fs::read_to_string(test_json_path).unwrap();
  let database_data = serde_json::from_str::<DatabaseData>(&json_content).unwrap();

  let mut id_mapping: HashMap<String, String> = HashMap::new();
  // database id
  id_mapping.insert(
    "17bf4626-c209-4b88-948d-be2ee423c6dd".to_string(),
    "00000000-0000-0000-0000-000000000000".to_string(),
  );
  // view id
  id_mapping.insert(
    "50dfa70a-d53c-4b7d-8b49-6194aadbac2a".to_string(),
    "11111111-1111-1111-1111-111111111111".to_string(),
  );
  // row ids
  id_mapping.insert(
    "be970ba6-9576-4e5f-a15d-c9a04d589a57".to_string(),
    "22222222-2222-2222-2222-222222222222".to_string(),
  );
  id_mapping.insert(
    "7f7a7058-2bf9-4bce-ba17-0bfa3255462b".to_string(),
    "33333333-3333-3333-3333-333333333333".to_string(),
  );
  id_mapping.insert(
    "cd4b8daa-868b-4a7e-ba17-03ccb93be3d2".to_string(),
    "44444444-4444-4444-4444-444444444444".to_string(),
  );

  let remapper = DatabaseCollabRemapper::new(id_mapping);
  let remapped_data = remapper.remap_database_data(database_data).unwrap();
  assert_eq!(remapped_data.rows.len(), 3);
  assert_eq!(
    remapped_data.database_id.to_string(),
    "00000000-0000-0000-0000-000000000000"
  );
  assert_eq!(remapped_data.row_metas.len(), 3);

  let remapped_row_id = Uuid::parse_str("22222222-2222-2222-2222-222222222222").unwrap();
  let remapped_meta = remapped_data.row_metas.get(&remapped_row_id).unwrap();

  assert_eq!(remapped_meta.icon_url, Some("🥀".to_string()));
  assert!(remapped_meta.cover.is_some());
  assert!(!remapped_meta.is_document_empty);

  let old_row_id = Uuid::parse_str("be970ba6-9576-4e5f-a15d-c9a04d589a57").unwrap();
  assert!(!remapped_data.row_metas.contains_key(&old_row_id));
}
