use crate::util::sync_unzip_asset;
use collab_importer::workspace::database_collab_remapper::DatabaseCollabRemapper;
use collab_importer::workspace::id_mapper::IdMapper;
use collab_importer::workspace::relation_map_parser::RelationMapParser;

#[tokio::test]
async fn test_parse_real_database_json() {
  let (_cleaner, unzip_path) = sync_unzip_asset("2025-07-16_22-15-54").await.unwrap();
  let json_path =
    unzip_path.join("collab_jsons/databases/6cbe3ff3-7b3a-4d3b-9eec-f0d1e0a8b8c3.json");
  let json_content = std::fs::read_to_string(&json_path).unwrap();
  let json_value: serde_json::Value = serde_json::from_str(&json_content).unwrap();

  let relation_map_path = unzip_path.join("relation_map.json");
  let parser = RelationMapParser {};
  let relation_map = parser
    .parse_relation_map(&relation_map_path.to_string_lossy())
    .await
    .unwrap();
  let id_mapper = IdMapper::new(&relation_map).unwrap();

  let view_id_mapping = id_mapper.get_id_map_as_strings();
  let remapper = DatabaseCollabRemapper::new(json_value, view_id_mapping);
  let database = remapper.build_database().await.unwrap();

  let original_database_id = "0730a32c-5a52-43fb-8e68-ee73287ebf69";
  if let Some(new_database_id) = id_mapper.get_new_id(original_database_id) {
    assert_eq!(database.get_database_id().unwrap(), new_database_id);
  }

  let views = database.get_all_views();
  assert_eq!(views.len(), 2);

  for view in &views {
    if view.name == "Untitled" {
      let original_view_id = "6cbe3ff3-7b3a-4d3b-9eec-f0d1e0a8b8c3";
      if let Some(new_view_id) = id_mapper.get_new_id(original_view_id) {
        assert_eq!(view.id, new_view_id);
      }
    }
  }

  let fields = database.get_all_fields();
  assert_eq!(fields.len(), 5);

  let database_data = database.get_database_data(20, false).await.unwrap();
  assert_eq!(database_data.rows.len(), 5);

  let original_uuids = [
    "0730a32c-5a52-43fb-8e68-ee73287ebf69",
    "6cbe3ff3-7b3a-4d3b-9eec-f0d1e0a8b8c3",
    "62d9aaca-2b76-43a1-b345-a1cd566ef278",
    "a334ccb6-684b-4197-99f9-c79cee2b9f60",
    "83d3ddd3-9778-48ec-bd69-9493b13c11ea",
  ];

  let data_json = serde_json::to_string(&database_data).unwrap();

  for original_uuid in &original_uuids {
    if id_mapper.get_new_id(original_uuid).is_some() {
      assert!(
        !data_json.contains(original_uuid),
        "original uuid {} should not be present in database",
        original_uuid
      );
    }
  }

  for original_uuid in &original_uuids {
    if let Some(new_uuid) = id_mapper.get_new_id(original_uuid) {
      assert!(
        data_json.contains(&new_uuid.to_string()),
        "new uuid {} should be present in database",
        new_uuid
      );
    }
  }

  let rows = database_data.rows;
  assert_eq!(rows.len(), 5);

  for row in &rows {
    let row_id_str = row.id.to_string();
    for original_uuid in &original_uuids {
      assert!(
        row_id_str != *original_uuid,
        "row id {} should not contain original uuid {}",
        row_id_str,
        original_uuid
      );
    }

    if let Some(new_row_id) = id_mapper.get_new_id(&row_id_str) {
      assert_ne!(
        row_id_str,
        new_row_id.to_string(),
        "row id should be mapped correctly"
      );
    }

    assert_eq!(
      &row.database_id.to_string(),
      &database.get_database_id().unwrap().to_string(),
      "row database_id should match database id"
    );

    let row_json = serde_json::to_string(&row).unwrap();
    for original_uuid in &original_uuids {
      if id_mapper.get_new_id(original_uuid).is_some() {
        assert!(
          !row_json.contains(original_uuid),
          "row data should not contain original uuid {}",
          original_uuid
        );
      }
    }
  }
}
