use std::collections::HashMap;

use collab_document::blocks::{BlockAction, BlockActionPayload, BlockActionType};
use serde_json::json;

use crate::blocks::block_test_core::{BlockTestCore, TEXT_BLOCK_TYPE};
use crate::util::try_decode_from_encode_collab;

#[tokio::test]
async fn create_default_document_test() {
  let test = BlockTestCore::new().await;
  let document_data = test.get_document_data();
  let page = test.get_page();
  let page_id = page.id.as_str();
  assert_eq!(page_id.to_string(), document_data.page_id);
  assert_eq!(page.ty, "page");
  let page_children = test.get_block_children(page_id);
  assert_eq!(page_children.len(), 1);
  let first_block_id = page_children[0].id.as_str();
  let first_block = test.get_block(first_block_id);
  assert_eq!(first_block.ty, TEXT_BLOCK_TYPE);
}

#[tokio::test]
async fn open_document_test() {
  let test = BlockTestCore::new().await;
  let page = test.get_page();
  let page_id = page.id.as_str();
  let collab = test.collab;
  let test = BlockTestCore::open(collab, test.db);
  let page = test.get_page();
  let reopened_page_id = page.id.as_str();
  assert_eq!(page_id, reopened_page_id);
}

#[tokio::test]
async fn subscribe_insert_change_test() {
  let mut test = BlockTestCore::new().await;
  test.subscribe(|_e, _| {
    // do nothing
  });
  let page = test.get_page();
  let page_id = page.id.as_str();
  let text = "Hello World".to_string();
  test.insert_text_block(text, page_id, None);
}

#[tokio::test]
async fn subscribe_update_change_test() {
  let mut test = BlockTestCore::new().await;
  test.subscribe(|_e, _| {
    // do nothing
  });
  let page = test.get_page();
  let page_id = page.id.as_str();
  let mut data = HashMap::new();
  data.insert("text".to_string(), json!("Hello World Updated"));
  test.update_block_data(page_id, data);
}

#[tokio::test]
async fn subscribe_delete_change_test() {
  let mut test = BlockTestCore::new().await;
  test.subscribe(|_e, _| {
    // do nothing
  });
  let page = test.get_page();
  let page_id = page.id.as_str();
  let page_children = test.get_block_children(page_id);
  let first_block_id = page_children[0].id.as_str();
  test.delete_block(first_block_id);
}

#[tokio::test]
async fn insert_block_test() {
  let test = BlockTestCore::new().await;
  let page = test.get_page();
  let page_id = page.id.as_str();
  let page_children = test.get_block_children(page_id);
  let original_first_block_id = page_children[0].id.as_str();
  let text = "Hello World".to_string();
  // insert before original_first_block
  let first_block = test.insert_text_block(text, page_id, None);
  let page_children = test.get_block_children(page_id);
  assert_eq!(page_children.len(), 2);
  let first_block_id = page_children[0].id.as_str();
  assert_eq!(first_block_id, first_block.id.as_str());
  assert_eq!(first_block.ty, TEXT_BLOCK_TYPE);
  assert_eq!(original_first_block_id, page_children[1].id.as_str());
  // insert after original_first_block
  let text = "Hello World 2".to_string();
  let last_block = test.insert_text_block(text, page_id, Some(original_first_block_id.to_string()));
  let page_children = test.get_block_children(page_id);
  assert_eq!(page_children.len(), 3);
  let last_block_id = page_children[2].id.as_str();
  assert_eq!(last_block_id, last_block.id.as_str());
  assert_eq!(last_block.ty, TEXT_BLOCK_TYPE);
  assert_eq!(original_first_block_id, page_children[1].id.as_str());
}

#[tokio::test]
async fn delete_block_test() {
  let test = BlockTestCore::new().await;
  let page = test.get_page();
  let page_id = page.id.as_str();
  let text = "Hello World".to_string();
  let first_block = test.insert_text_block(text, page_id, None);
  let text = "Hello World 2".to_string();
  test.insert_text_block(text, &first_block.id, None);
  let page_children = test.get_block_children(page_id);
  let first_block_children = test.get_block_children(&first_block.id);
  assert_eq!(page_children.len(), 2);
  assert_eq!(first_block_children.len(), 1);

  test.delete_block(&first_block.id);
  let page_children = test.get_block_children(page_id);
  assert_eq!(page_children.len(), 1);
  try_decode_from_encode_collab(&test.document);
}

#[tokio::test]
async fn move_block_test() {
  let test = BlockTestCore::new().await;
  let page = test.get_page();
  let page_id = page.id.as_str();
  let text = "Hello World".to_string();
  let first_block = test.insert_text_block(text, page_id, None);
  let first_block_id = first_block.id.as_str();
  let text = "Hello World 2".to_string();
  let first_block_child = test.insert_text_block(text, first_block_id, None);
  let first_block_child_id = first_block_child.id.as_str();
  let text = "Hello World 3".to_string();
  let second_block = test.insert_text_block(text, page_id, None);
  let second_block_id = second_block.id.as_str();
  // move first_block_child to page and after second_block
  test.move_block(
    first_block_child_id,
    page_id,
    Some(second_block_id.to_string()),
  );
  let page_children = test.get_block_children(page_id);
  assert_eq!(page_children.len(), 4);
  let first_block_children = test.get_block_children(first_block_id);
  assert_eq!(first_block_children.len(), 0);
  // move first_block_child to second_block
  test.move_block(first_block_child_id, second_block_id, None);
  let second_block_children = test.get_block_children(second_block_id);
  assert_eq!(second_block_children.len(), 1);
  let page_children = test.get_block_children(page_id);
  assert_eq!(page_children.len(), 3);
  // move second_block before first_block
  test.move_block(second_block_id, page_id, None);
  let page_children = test.get_block_children(page_id);
  assert_eq!(page_children[0].id, second_block_id);

  // move second_block after first_block
  test.move_block(second_block_id, page_id, Some(first_block_id.to_string()));
  let page_children = test.get_block_children(page_id);
  assert_eq!(page_children[0].id, first_block_id);
  assert_eq!(page_children[1].id, second_block_id);

  try_decode_from_encode_collab(&test.document);
}

#[tokio::test]
async fn update_block_data_test() {
  let test = BlockTestCore::new().await;
  let page = test.get_page();
  let page_id = page.id.as_str();
  let page_children = test.get_block_children(page_id);
  let block_id = page_children[0].id.as_str();
  let update_text = "Hello World Updated".to_string();
  let mut update_data = HashMap::new();
  update_data.insert("text".to_string(), json!(update_text));
  test.update_block_data(block_id, update_data);
  let block = test.get_block(block_id);
  let mut expected_data = HashMap::new();
  expected_data.insert("text".to_string(), json!(update_text));

  assert_eq!(block.data, expected_data);
  try_decode_from_encode_collab(&test.document);
}

#[tokio::test]
async fn apply_actions_test() {
  let test = BlockTestCore::new().await;
  let page = test.get_page();
  let page_id = page.id.as_str();
  let text = "Hello World".to_string();
  let insert_action = test.get_insert_action(text, page_id, None);
  test.apply_action(vec![insert_action]);
  let page_children = test.get_block_children(page_id);
  assert_eq!(page_children.len(), 2);
  let first_block_id = page_children[0].id.as_str();
  let last_block_id = page_children[1].id.as_str();

  let update_text = "Hello World Updated".to_string();
  let update_action = test.get_update_action(update_text.clone(), first_block_id);
  test.apply_action(vec![update_action]);
  let block = test.get_block(first_block_id);
  let mut expected_data = HashMap::new();
  expected_data.insert("delta".to_string(), json!([{ "insert": update_text }]));
  assert_eq!(block.data, expected_data);

  let move_action = test.get_move_action(first_block_id, page_id, Some(last_block_id.to_string()));
  test.apply_action(vec![move_action]);
  let page_children = test.get_block_children(page_id);
  assert_eq!(page_children.len(), 2);

  let delete_action = test.get_delete_action(first_block_id);
  test.apply_action(vec![delete_action]);
  let page_children = test.get_block_children(page_id);
  assert_eq!(page_children.len(), 1);
}

#[tokio::test]
async fn apply_insert_block_action_without_parent_id_test() {
  let test = BlockTestCore::new().await;
  let page = test.get_page();
  let page_id = page.id.as_str();
  let text = "Hello World".to_string();
  let mut insert_action = test.get_insert_action(text, page_id, None);
  insert_action.payload.parent_id = None;
  test.apply_action(vec![insert_action]);
  let page_children = test.get_block_children(page_id);
  assert_eq!(page_children.len(), 2);
}

#[tokio::test]
async fn apply_block_actions_without_block_test() {
  let test = BlockTestCore::new().await;
  let page = test.get_page();
  let page_id = page.id.as_str();
  let document_data = test.get_document_data();

  let payload = BlockActionPayload {
    block: None,
    prev_id: None,
    parent_id: Some(page_id.to_string()),
    delta: None,
    text_id: None,
  };
  let actions = vec![
    BlockAction {
      action: BlockActionType::Insert,
      payload: payload.clone(),
    },
    BlockAction {
      action: BlockActionType::Update,
      payload: payload.clone(),
    },
    BlockAction {
      action: BlockActionType::Delete,
      payload: payload.clone(),
    },
    BlockAction {
      action: BlockActionType::Move,
      payload,
    },
  ];
  test.apply_action(actions);
  // nothing should happen
  assert_eq!(document_data, test.get_document_data());
  try_decode_from_encode_collab(&test.document);
}
