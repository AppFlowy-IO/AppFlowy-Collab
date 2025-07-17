use collab_importer::workspace::id_mapper::IdMapper;
use collab_importer::workspace::relation_map_parser::RelationMapParser;
use collab_importer::workspace::workspace_database_remapper::WorkspaceDatabaseRemapper;

#[tokio::test]
async fn test_workspace_database_remapper() {
  let relation_map_path = "tests/asset/2025-07-17_16-37-11/relation_map.json";
  let parser = RelationMapParser {};
  let relation_map = parser.parse_relation_map(relation_map_path).await.unwrap();
  let id_mapper = IdMapper::new(&relation_map);

  let view_id_mapping = id_mapper.id_map.clone();

  let workspace_database_json = serde_json::json!({
    "databases": relation_map.workspace_database_meta
  });

  let remapper = WorkspaceDatabaseRemapper::new(workspace_database_json, view_id_mapping);

  let workspace_database_data = remapper.build_workspace_database_data().unwrap();

  let original_uuids = [
    "0730a32c-5a52-43fb-8e68-ee73287ebf69",
    "6cbe3ff3-7b3a-4d3b-9eec-f0d1e0a8b8c3",
    "db51cd93-138a-4b66-82c6-141fa7af5af8",
  ];

  assert_eq!(workspace_database_data.databases.len(), 1);
  let database_meta = &workspace_database_data.databases[0];
  assert_eq!(database_meta.view_ids.len(), 2);

  let json_string = serde_json::to_string(&workspace_database_data).unwrap();

  for original_uuid in &original_uuids {
    assert!(
      !json_string.contains(original_uuid),
      "original uuid {} should not be present in workspace database data",
      original_uuid
    );

    if let Some(new_uuid) = id_mapper.get_new_id(original_uuid) {
      assert!(
        json_string.contains(new_uuid),
        "new uuid {} should be present in workspace database data",
        new_uuid
      );
    }
  }

  let workspace_database = remapper
    .build_workspace_database("test-workspace-database-id")
    .unwrap();
  let all_database_meta = workspace_database.get_all_database_meta();
  assert_eq!(all_database_meta.len(), 1);
  assert_eq!(all_database_meta[0].linked_views.len(), 2);
}
