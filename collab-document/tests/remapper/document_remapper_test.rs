use collab::core::collab::{CollabOptions, DataSource};
use collab::core::origin::CollabOrigin;
use collab::preclude::*;
use collab_document::document::Document;
use collab_document::document_remapper::DocumentCollabRemapper;
use std::collections::HashMap;
use std::fs;

fn doc_state_to_document(doc_state: &Vec<u8>, doc_id: &str, user_id: &str) -> Document {
  let client_id = user_id.parse::<u64>().unwrap_or(0);
  let options = CollabOptions::new(doc_id.to_string(), client_id)
    .with_data_source(DataSource::DocStateV1(doc_state.clone()));
  let collab =
    Collab::new_with_options(CollabOrigin::Empty, options).expect("Failed to create collab");
  Document::open(collab).expect("Failed to open document")
}

#[test]
fn test_remap_collab_with_mentioned_page_ids() {
  let test_collab_path = "tests/assets/mention_page/b29ee07f-c7b2-4b24-a8c6-5cd6d8ba1213.collab";
  let doc_state = fs::read(test_collab_path).expect("Failed to read test collab file");

  let mut id_mapping: HashMap<String, String> = HashMap::new();
  // "aa3e167f-d36b-44cf-a8d2-0105a66f184c",
  // "c09342b5-5c92-4eda-9cc3-e84374cc87e9",
  // "faae358d-d4b0-4426-b634-350a62a25f26"
  // "d5db7722-8919-4e9b-ac2d-8d054015dcb2"
  id_mapping.insert(
    "b29ee07f-c7b2-4b24-a8c6-5cd6d8ba1213".to_string(),
    "parent_id_1".to_string(),
  );
  id_mapping.insert(
    "aa3e167f-d36b-44cf-a8d2-0105a66f184c".to_string(),
    "child_id_1".to_string(),
  );
  id_mapping.insert(
    "c09342b5-5c92-4eda-9cc3-e84374cc87e9".to_string(),
    "child_id_2".to_string(),
  );
  id_mapping.insert(
    "faae358d-d4b0-4426-b634-350a62a25f26".to_string(),
    "child_id_3".to_string(),
  );
  id_mapping.insert(
    "d5db7722-8919-4e9b-ac2d-8d054015dcb2".to_string(),
    "child_id_4".to_string(),
  );

  let remapper = DocumentCollabRemapper::new(id_mapping);

  let doc_id = "test_doc_id";
  let user_id = "123456";

  let remapped_state = remapper
    .remap_collab_doc_state(doc_id, user_id, &doc_state)
    .unwrap();

  let remapped_doc = doc_state_to_document(&remapped_state, doc_id, user_id);
  let remapped_data = remapped_doc.get_document_data().unwrap();

  let mut found_remapped_mentions = 0;
  if let Some(text_map) = &remapped_data.meta.text_map {
    for (_, text_delta_str) in text_map {
      if let Ok(text_deltas) = serde_json::from_str::<Vec<serde_json::Value>>(text_delta_str) {
        for delta in text_deltas {
          if let Some(attributes) = delta.get("attributes") {
            if let Some(mention) = attributes.get("mention") {
              if let Some(mention_str) = mention.as_str() {
                if let Ok(mention_obj) = serde_json::from_str::<serde_json::Value>(mention_str) {
                  if let Some(page_id) = mention_obj.get("page_id").and_then(|v| v.as_str()) {
                    println!("Found mention with page_id: {}", page_id);
                    if page_id == "child_id_1"
                      || page_id == "child_id_2"
                      || page_id == "child_id_3"
                      || page_id == "child_id_4"
                    {
                      found_remapped_mentions += 1;
                      println!("Found remapped mention page_id: {}", page_id);
                    }
                  }
                }
              }
            }
          }
        }
      }
    }
  }

  assert!(
    found_remapped_mentions == 4,
    "Should have found remapped page IDs in mentions"
  );
}

#[test]
fn test_remap_collab_with_inline_database() {
  let test_collab_path = "tests/assets/inline_database/b29ee07f-c7b2-4b24-a8c6-5cd6d8ba1213.collab";
  let doc_state = fs::read(test_collab_path).expect("Failed to read test collab file");

  let mut id_mapping: HashMap<String, String> = HashMap::new();
  id_mapping.insert(
    "d5db7722-8919-4e9b-ac2d-8d054015dcb2".to_string(),
    "new_parent_id".to_string(),
  );
  id_mapping.insert(
    "131c2815-b5c5-4cb2-97ef-5042f3cb8866".to_string(),
    "new_view_id".to_string(),
  );

  let remapper = DocumentCollabRemapper::new(id_mapping);

  let doc_id = "test_doc_id";
  let user_id = "123456";

  let remapped_state = remapper
    .remap_collab_doc_state(doc_id, user_id, &doc_state)
    .unwrap();
  let remapped_doc = doc_state_to_document(&remapped_state, doc_id, user_id);
  let remapped_data = remapped_doc.get_document_data().unwrap();

  let mut found_remapped_database_parent_id = false;
  let mut found_remapped_database_view_id = false;
  for (_, block) in &remapped_data.blocks {
    if ["grid", "board", "calendar"].contains(&block.ty.as_str()) {
      if let Some(parent_id) = block.data.get("parent_id").and_then(|v| v.as_str()) {
        if parent_id == "new_parent_id" {
          found_remapped_database_parent_id = true;
        }
      }

      if let Some(view_id) = block.data.get("view_id").and_then(|v| v.as_str()) {
        if view_id == "new_view_id" {
          found_remapped_database_view_id = true;
        }
      }
    }
  }

  assert!(
    found_remapped_database_parent_id && found_remapped_database_view_id,
    "Should have found remapped IDs in inline database blocks"
  );
}
