use collab_importer::workspace::relation_map_parser::RelationMapParser;
use crate::util::sync_unzip_asset;

#[tokio::test]
async fn test_parse_with_valid_relation_map() {
  let parser = RelationMapParser {};
  let (_cleaner, unzip_path) = sync_unzip_asset("2025-07-16_22-15-54")
    .await
    .unwrap();
  let test_file_path = unzip_path.join("relation_map.json");

  let result = parser
    .parse_relation_map(&test_file_path.to_string_lossy())
    .await;
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
