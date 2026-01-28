use crate::util::sync_unzip_asset;
use collab::importer::workspace::document_collab_remapper::DocumentCollabRemapper;
use collab::importer::workspace::id_mapper::IdMapper;
use collab::importer::workspace::relation_map_parser::RelationMapParser;

#[tokio::test]
async fn test_parse_real_document_json() {
  let (_cleaner, unzip_path) = sync_unzip_asset("2025-07-16_22-15-54").await.unwrap();
  let json_path =
    unzip_path.join("collab_jsons/documents/b8f96497-c880-4fea-8232-c31d57daab83.json");
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

  let remapper = DocumentCollabRemapper::new(json_value, view_id_mapping);

  let document_data = remapper.build_document_data().unwrap();

  let original_uuids = [
    "b68f3000-6f31-452f-b781-db3a65aced1f",
    "6cbe3ff3-7b3a-4d3b-9eec-f0d1e0a8b8c3",
    "0a0fd09b-31ed-4cb6-814d-34280d65c5ef",
    "d0b0104e-996d-498b-b644-0556ebe6a37a",
  ];

  assert_eq!(document_data.page_id, "wl_3CTczV-");
  assert_eq!(document_data.blocks.len(), 16);

  assert!(document_data.blocks.contains_key("wl_3CTczV-"));
  assert!(document_data.blocks.contains_key("GZZKIAfmPj"));
  assert!(document_data.blocks.contains_key("dFhgpeeWqS"));

  assert!(document_data.meta.children_map.contains_key("9HaPo6SKKI"));
  assert_eq!(document_data.meta.children_map["9HaPo6SKKI"].len(), 14);

  let text_map = document_data.meta.text_map.as_ref().unwrap();
  assert!(text_map.contains_key("7uf16a95nA"));

  let text_map_content = serde_json::to_string(text_map).unwrap();

  for original_uuid in &original_uuids {
    assert!(
      !text_map_content.contains(original_uuid),
      "Original UUID {} should not be present in DocumentData",
      original_uuid
    );

    if let Some(new_uuid) = id_mapper.get_new_id(original_uuid) {
      assert!(
        text_map_content.contains(&new_uuid.to_string()),
        "New UUID {} should be present in DocumentData",
        new_uuid
      );
    }
  }

  let document = remapper.build_document(&uuid::Uuid::new_v4()).unwrap();
  assert_eq!(document.get_page_id().unwrap(), "wl_3CTczV-");

  let doc_data_from_document = document.get_document_data().unwrap();
  assert_eq!(doc_data_from_document.blocks.len(), 16);
  assert_eq!(doc_data_from_document.page_id, document_data.page_id);
}
