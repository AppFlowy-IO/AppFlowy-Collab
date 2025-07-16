use collab_importer::workspace::relation_map_parser::RelationMapParser;

#[tokio::test]
async fn test_parse_with_valid_relation_map() {
  let parser = RelationMapParser {};
  let test_file_path = "tests/asset/relation_map/2025_Jul_16_relation_map.json";

  let result = parser.parse_relation_map(test_file_path).await;
  let relation_map = result.unwrap();
  assert_eq!(
    relation_map.workspace_id,
    "87805052-485c-4d0d-a69d-e32e0428bbc0"
  );
  assert_eq!(relation_map.export_timestamp, 1752675354);
  assert_eq!(relation_map.views.len(), 7);
  assert_eq!(relation_map.collab_objects.len(), 12);
  assert_eq!(relation_map.dependencies.len(), 10);
}
