use crate::util::DocumentTest;
use collab::document::blocks::{Block, BlockAction, BlockActionPayload, BlockActionType};
use collab::document::document::EmbeddedDatabaseBlock;
use nanoid::nanoid;
use serde_json::json;
use std::collections::HashMap;
use uuid::Uuid;

fn test_uuid(n: u8) -> Uuid {
  Uuid::from_bytes([n, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, n])
}

fn create_database_block(
  block_type: &str,
  parent: &str,
  view_id: Option<&str>,
  view_ids: Option<Vec<&str>>,
  parent_id: Option<&str>,
  database_id: Option<&str>,
) -> Block {
  let mut data = HashMap::new();

  if let Some(vid) = view_id {
    data.insert("view_id".to_string(), json!(vid));
  }
  if let Some(vids) = view_ids {
    data.insert("view_ids".to_string(), json!(vids));
  }
  if let Some(pid) = parent_id {
    data.insert("parent_id".to_string(), json!(pid));
  }
  if let Some(dbid) = database_id {
    data.insert("database_id".to_string(), json!(dbid));
  }

  Block {
    id: nanoid!(10),
    ty: block_type.to_string(),
    parent: parent.to_string(),
    children: nanoid!(10),
    external_id: None,
    external_type: None,
    data,
  }
}

fn insert_block(test: &mut DocumentTest, block: Block, page_id: &str) {
  let action = BlockAction {
    action: BlockActionType::Insert,
    payload: BlockActionPayload {
      prev_id: None,
      parent_id: Some(page_id.to_string()),
      block: Some(block),
      delta: None,
      text_id: None,
    },
  };
  test.document.apply_action(vec![action]).unwrap();
}

#[test]
fn get_embedded_database_blocks_returns_empty_for_document_without_databases() {
  let test = DocumentTest::new(1, "test-doc");

  let databases = test.document.get_embedded_database_blocks();

  assert!(databases.is_empty());
}

#[test]
fn get_embedded_database_blocks_extracts_grid_block() {
  let mut test = DocumentTest::new(1, "test-doc");
  let page_id = test.document.get_page_id().unwrap();

  let view_id = test_uuid(1).to_string();
  let parent_id = test_uuid(2).to_string();
  let database_id = test_uuid(3).to_string();

  let grid_block = create_database_block(
    "grid",
    &page_id,
    Some(&view_id),
    None,
    Some(&parent_id),
    Some(&database_id),
  );
  insert_block(&mut test, grid_block, &page_id);

  let databases = test.document.get_embedded_database_blocks();

  assert_eq!(databases.len(), 1);
  assert_eq!(databases[0].block_type, "grid");
  assert_eq!(databases[0].view_ids.len(), 1);
  assert_eq!(databases[0].view_ids[0], test_uuid(1));
  assert_eq!(databases[0].parent_id, Some(test_uuid(2)));
  assert_eq!(databases[0].database_id, Some(test_uuid(3)));
}

#[test]
fn get_embedded_database_blocks_extracts_board_block() {
  let mut test = DocumentTest::new(1, "test-doc");
  let page_id = test.document.get_page_id().unwrap();

  let view_id = test_uuid(1).to_string();
  let board_block = create_database_block("board", &page_id, Some(&view_id), None, None, None);
  insert_block(&mut test, board_block, &page_id);

  let databases = test.document.get_embedded_database_blocks();

  assert_eq!(databases.len(), 1);
  assert_eq!(databases[0].block_type, "board");
}

#[test]
fn get_embedded_database_blocks_extracts_calendar_block() {
  let mut test = DocumentTest::new(1, "test-doc");
  let page_id = test.document.get_page_id().unwrap();

  let view_id = test_uuid(1).to_string();
  let calendar_block =
    create_database_block("calendar", &page_id, Some(&view_id), None, None, None);
  insert_block(&mut test, calendar_block, &page_id);

  let databases = test.document.get_embedded_database_blocks();

  assert_eq!(databases.len(), 1);
  assert_eq!(databases[0].block_type, "calendar");
}

#[test]
fn get_embedded_database_blocks_extracts_multiple_databases() {
  let mut test = DocumentTest::new(1, "test-doc");
  let page_id = test.document.get_page_id().unwrap();

  let grid_block = create_database_block(
    "grid",
    &page_id,
    Some(&test_uuid(1).to_string()),
    None,
    None,
    None,
  );
  let board_block = create_database_block(
    "board",
    &page_id,
    Some(&test_uuid(2).to_string()),
    None,
    None,
    None,
  );
  let calendar_block = create_database_block(
    "calendar",
    &page_id,
    Some(&test_uuid(3).to_string()),
    None,
    None,
    None,
  );

  insert_block(&mut test, grid_block, &page_id);
  insert_block(&mut test, board_block, &page_id);
  insert_block(&mut test, calendar_block, &page_id);

  let databases = test.document.get_embedded_database_blocks();

  assert_eq!(databases.len(), 3);
}

#[test]
fn get_embedded_database_blocks_ignores_non_database_blocks() {
  let mut test = DocumentTest::new(1, "test-doc");
  let page_id = test.document.get_page_id().unwrap();

  // Insert a paragraph block
  let paragraph_block = Block {
    id: nanoid!(10),
    ty: "paragraph".to_string(),
    parent: page_id.clone(),
    children: nanoid!(10),
    external_id: None,
    external_type: None,
    data: HashMap::new(),
  };
  insert_block(&mut test, paragraph_block, &page_id);

  let databases = test.document.get_embedded_database_blocks();

  assert!(databases.is_empty());
}

#[test]
fn get_embedded_database_blocks_extracts_view_ids_array() {
  let mut test = DocumentTest::new(1, "test-doc");
  let page_id = test.document.get_page_id().unwrap();

  let view_ids = vec![
    test_uuid(1).to_string(),
    test_uuid(2).to_string(),
    test_uuid(3).to_string(),
  ];
  let view_ids_refs: Vec<&str> = view_ids.iter().map(|s| s.as_str()).collect();

  let grid_block = create_database_block("grid", &page_id, None, Some(view_ids_refs), None, None);
  insert_block(&mut test, grid_block, &page_id);

  let databases = test.document.get_embedded_database_blocks();

  assert_eq!(databases.len(), 1);
  assert_eq!(databases[0].view_ids.len(), 3);
  assert_eq!(databases[0].view_ids[0], test_uuid(1));
  assert_eq!(databases[0].view_ids[1], test_uuid(2));
  assert_eq!(databases[0].view_ids[2], test_uuid(3));
}

#[test]
fn get_embedded_database_view_ids_returns_all_view_ids() {
  let mut test = DocumentTest::new(1, "test-doc");
  let page_id = test.document.get_page_id().unwrap();

  let grid_block = create_database_block(
    "grid",
    &page_id,
    Some(&test_uuid(1).to_string()),
    None,
    None,
    None,
  );
  let board_block = create_database_block(
    "board",
    &page_id,
    Some(&test_uuid(2).to_string()),
    None,
    None,
    None,
  );

  insert_block(&mut test, grid_block, &page_id);
  insert_block(&mut test, board_block, &page_id);

  let view_ids = test.document.get_embedded_database_view_ids();

  assert_eq!(view_ids.len(), 2);
  assert!(view_ids.contains(&test_uuid(1)));
  assert!(view_ids.contains(&test_uuid(2)));
}

#[test]
fn embedded_database_block_is_inline_database() {
  let document_id = test_uuid(5);
  let info = EmbeddedDatabaseBlock {
    block_id: "test-block".to_string(),
    block_type: "grid".to_string(),
    view_ids: vec![test_uuid(1)],
    parent_id: Some(document_id),
    database_id: Some(test_uuid(10)),
  };

  assert!(info.is_inline_database(&document_id));
  assert!(!info.is_linked_database(&document_id));
}

#[test]
fn embedded_database_block_is_linked_database() {
  let document_id = test_uuid(5);
  let original_db_id = test_uuid(6);
  let info = EmbeddedDatabaseBlock {
    block_id: "test-block".to_string(),
    block_type: "grid".to_string(),
    view_ids: vec![test_uuid(1)],
    parent_id: Some(original_db_id),
    database_id: Some(test_uuid(10)),
  };

  assert!(!info.is_inline_database(&document_id));
  assert!(info.is_linked_database(&document_id));
}

#[test]
fn embedded_database_block_handles_none_parent_id() {
  let document_id = test_uuid(5);
  let info = EmbeddedDatabaseBlock {
    block_id: "test-block".to_string(),
    block_type: "grid".to_string(),
    view_ids: vec![test_uuid(1)],
    parent_id: None,
    database_id: None,
  };

  assert!(!info.is_inline_database(&document_id));
  assert!(!info.is_linked_database(&document_id));
}

// ============================================================================
// Sub-page tests
// ============================================================================

fn create_sub_page_block(parent: &str, view_id: &str) -> Block {
  let mut data = HashMap::new();
  data.insert("view_id".to_string(), json!(view_id));

  Block {
    id: nanoid!(10),
    ty: "sub_page".to_string(),
    parent: parent.to_string(),
    children: nanoid!(10),
    external_id: None,
    external_type: None,
    data,
  }
}

#[test]
fn get_sub_page_ids_returns_empty_for_document_without_sub_pages() {
  let test = DocumentTest::new(1, "test-doc");

  let sub_page_ids = test.document.get_sub_page_ids();

  assert!(sub_page_ids.is_empty());
}

#[test]
fn get_sub_page_ids_extracts_single_sub_page() {
  let mut test = DocumentTest::new(1, "test-doc");
  let page_id = test.document.get_page_id().unwrap();

  let view_id = test_uuid(1).to_string();
  let sub_page_block = create_sub_page_block(&page_id, &view_id);
  insert_block(&mut test, sub_page_block, &page_id);

  let sub_page_ids = test.document.get_sub_page_ids();

  assert_eq!(sub_page_ids.len(), 1);
  assert_eq!(sub_page_ids[0], test_uuid(1));
}

#[test]
fn get_sub_page_ids_extracts_multiple_sub_pages() {
  let mut test = DocumentTest::new(1, "test-doc");
  let page_id = test.document.get_page_id().unwrap();

  let sub_page_1 = create_sub_page_block(&page_id, &test_uuid(1).to_string());
  let sub_page_2 = create_sub_page_block(&page_id, &test_uuid(2).to_string());
  let sub_page_3 = create_sub_page_block(&page_id, &test_uuid(3).to_string());

  insert_block(&mut test, sub_page_1, &page_id);
  insert_block(&mut test, sub_page_2, &page_id);
  insert_block(&mut test, sub_page_3, &page_id);

  let sub_page_ids = test.document.get_sub_page_ids();

  assert_eq!(sub_page_ids.len(), 3);
  assert!(sub_page_ids.contains(&test_uuid(1)));
  assert!(sub_page_ids.contains(&test_uuid(2)));
  assert!(sub_page_ids.contains(&test_uuid(3)));
}

#[test]
fn get_sub_page_ids_ignores_non_sub_page_blocks() {
  let mut test = DocumentTest::new(1, "test-doc");
  let page_id = test.document.get_page_id().unwrap();

  // Insert a paragraph block
  let paragraph_block = Block {
    id: nanoid!(10),
    ty: "paragraph".to_string(),
    parent: page_id.clone(),
    children: nanoid!(10),
    external_id: None,
    external_type: None,
    data: HashMap::new(),
  };
  insert_block(&mut test, paragraph_block, &page_id);

  // Insert a grid block (database, not sub-page)
  let grid_block = create_database_block(
    "grid",
    &page_id,
    Some(&test_uuid(10).to_string()),
    None,
    None,
    None,
  );
  insert_block(&mut test, grid_block, &page_id);

  let sub_page_ids = test.document.get_sub_page_ids();

  assert!(sub_page_ids.is_empty());
}

#[test]
fn get_sub_page_ids_ignores_invalid_view_id() {
  let mut test = DocumentTest::new(1, "test-doc");
  let page_id = test.document.get_page_id().unwrap();

  // Create sub-page with invalid UUID
  let mut data = HashMap::new();
  data.insert("view_id".to_string(), json!("not-a-valid-uuid"));
  let invalid_sub_page = Block {
    id: nanoid!(10),
    ty: "sub_page".to_string(),
    parent: page_id.clone(),
    children: nanoid!(10),
    external_id: None,
    external_type: None,
    data,
  };
  insert_block(&mut test, invalid_sub_page, &page_id);

  let sub_page_ids = test.document.get_sub_page_ids();

  assert!(sub_page_ids.is_empty());
}

#[test]
fn get_sub_page_ids_ignores_missing_view_id() {
  let mut test = DocumentTest::new(1, "test-doc");
  let page_id = test.document.get_page_id().unwrap();

  // Create sub-page without view_id
  let sub_page_without_view_id = Block {
    id: nanoid!(10),
    ty: "sub_page".to_string(),
    parent: page_id.clone(),
    children: nanoid!(10),
    external_id: None,
    external_type: None,
    data: HashMap::new(),
  };
  insert_block(&mut test, sub_page_without_view_id, &page_id);

  let sub_page_ids = test.document.get_sub_page_ids();

  assert!(sub_page_ids.is_empty());
}

#[test]
fn get_sub_page_ids_mixed_with_database_blocks() {
  let mut test = DocumentTest::new(1, "test-doc");
  let page_id = test.document.get_page_id().unwrap();

  // Insert sub-pages
  let sub_page_1 = create_sub_page_block(&page_id, &test_uuid(1).to_string());
  let sub_page_2 = create_sub_page_block(&page_id, &test_uuid(2).to_string());

  // Insert database blocks
  let grid_block = create_database_block(
    "grid",
    &page_id,
    Some(&test_uuid(10).to_string()),
    None,
    None,
    None,
  );
  let board_block = create_database_block(
    "board",
    &page_id,
    Some(&test_uuid(11).to_string()),
    None,
    None,
    None,
  );

  insert_block(&mut test, sub_page_1, &page_id);
  insert_block(&mut test, grid_block, &page_id);
  insert_block(&mut test, sub_page_2, &page_id);
  insert_block(&mut test, board_block, &page_id);

  // Verify sub-pages are extracted correctly
  let sub_page_ids = test.document.get_sub_page_ids();
  assert_eq!(sub_page_ids.len(), 2);
  assert!(sub_page_ids.contains(&test_uuid(1)));
  assert!(sub_page_ids.contains(&test_uuid(2)));

  // Verify database blocks are extracted separately
  let database_view_ids = test.document.get_embedded_database_view_ids();
  assert_eq!(database_view_ids.len(), 2);
  assert!(database_view_ids.contains(&test_uuid(10)));
  assert!(database_view_ids.contains(&test_uuid(11)));
}
