use std::collections::HashMap;

use collab_document::blocks::{
  Block, BlockAction, BlockActionPayload, BlockActionType, EXTERNAL_TYPE_TEXT,
};
use nanoid::nanoid;
use serde_json::{json, to_value};

use crate::util::{
  apply_actions, create_document, delete_block, get_document_data, insert_block, move_block,
  update_block,
};

#[test]
fn create_block_test() {
  let doc_id = "1";
  let test = create_document(1, doc_id);
  let (page_id, blocks, children_map) = get_document_data(&test.document);

  let page_id = page_id.as_str();
  let page = &blocks[page_id];
  assert_eq!(page.id, page_id);
  assert_eq!(page.ty, "page");
  assert!(!page.data.is_empty());
  assert_eq!(&page.parent, "");
  let page_external_id = &page.external_id;
  let page_children_id = &page.children;
  let page_external_type = &page.external_type;

  assert!(&page_external_type.is_none());
  assert!(&page_external_id.is_none());

  let page_children = &children_map[page_children_id];
  assert_eq!(page_children.len(), 1);
  let first_child_id = page_children[0].as_str();
  assert_eq!(blocks[first_child_id].id, first_child_id);
  assert_eq!(blocks[first_child_id].parent, page_id.to_string());
  assert!(blocks[first_child_id].external_type.is_none());
}

#[test]
fn insert_block_test() {
  let doc_id = "1";
  let test = create_document(1, doc_id);

  let (page_id, blocks, children_map) = get_document_data(&test.document);

  let page_id = page_id.as_str();
  let page_children_id = blocks[page_id].children.as_str();
  let page_children = &children_map[page_children_id];
  let first_child_id = page_children[0].as_str();

  let block_external_id = nanoid!(10);
  let block_children_id = nanoid!(10);
  let block_id = nanoid!(10);

  let block = insert_block(
    &test.document,
    Block {
      parent: page_id.to_string(),
      ty: "text".to_string(),
      data: HashMap::new(),
      external_id: Some(block_external_id),
      external_type: Some(EXTERNAL_TYPE_TEXT.to_string()),
      id: block_id.to_string(),
      children: block_children_id.to_string(),
    },
    first_child_id.to_owned(),
  );
  let block_child = insert_block(
    &test.document,
    Block {
      parent: block_id.to_string(),
      ty: "text".to_string(),
      data: HashMap::new(),
      external_id: None,
      external_type: None,
      id: nanoid!(10),
      children: nanoid!(10),
    },
    "".to_string(),
  );
  let (page_id, _blocks, children_map) = get_document_data(&test.document);

  assert!(block_child.is_ok());
  assert!(block.is_ok());
  let block = block.unwrap();
  assert_eq!(block.parent, page_id);
  assert_eq!(block.children, block_children_id);
  assert_eq!(block.external_id, None);
  assert_eq!(block.external_type, None);
  assert_eq!(block.ty, "text");
  let block_child = block_child.unwrap();
  assert!(block_child.external_type.is_none());
  assert!(block_child.external_id.is_none());

  assert_eq!(
    children_map[&block_children_id][0].as_str(),
    &block_child.id
  );
  let page_children = &children_map[page_children_id];

  assert_eq!(page_children.len(), 2);
  assert_eq!(page_children[1].as_str(), &block_id);
}

#[test]
fn delete_block_test() {
  let doc_id = "1";
  let test = create_document(1, doc_id);

  let (page_id, blocks, children_map) = get_document_data(&test.document);

  let page_id = page_id.as_str();
  let page_children_id = blocks[page_id].children.as_str();
  let page_children = &children_map[page_children_id];
  let first_child_id = page_children[0].as_str();

  let block_external_id = nanoid!(10);
  let block_children_id = nanoid!(10);
  let block_id = nanoid!(10);
  let block = insert_block(
    &test.document,
    Block {
      parent: page_id.to_string(),
      ty: "text".to_string(),
      data: HashMap::new(),
      external_id: Some(block_external_id),
      external_type: Some(EXTERNAL_TYPE_TEXT.to_string()),
      id: block_id.to_string(),
      children: block_children_id.to_string(),
    },
    first_child_id.to_owned(),
  );
  insert_block(
    &test.document,
    Block {
      parent: block_id.to_string(),
      ty: "text".to_string(),
      data: HashMap::new(),
      external_id: Some(nanoid!(10)),
      external_type: Some(EXTERNAL_TYPE_TEXT.to_string()),
      id: nanoid!(10),
      children: nanoid!(10),
    },
    "".to_string(),
  )
  .unwrap();

  assert!(block.is_ok());
  let _block = delete_block(&test.document, &block.unwrap().id);
  let (_page_id, _blocks, children_map) = get_document_data(&test.document);

  assert!(children_map.get(&block_children_id).is_none());
  let page_children = &children_map[page_children_id];
  assert_eq!(page_children.len(), 1);
  assert_ne!(page_children[0].as_str(), &block_id);
  assert_eq!(page_children[0].as_str(), first_child_id);
}

#[test]
fn move_block_test() {
  let doc_id = "1";
  let test = create_document(1, doc_id);
  let (page_id, blocks, children_map) = get_document_data(&test.document);

  let page_id = page_id.as_str();
  let page_children_id = blocks[page_id].children.as_str();
  let page_children = &children_map[page_children_id];
  let first_child_id = page_children[0].as_str();

  let block = insert_block(
    &test.document,
    Block {
      parent: page_id.to_string(),
      ty: "text".to_string(),
      data: HashMap::new(),
      external_id: Some(nanoid!(10)),
      external_type: Some(EXTERNAL_TYPE_TEXT.to_string()),
      id: nanoid!(10),
      children: nanoid!(10),
    },
    first_child_id.to_owned(),
  )
  .unwrap();

  let block_id = block.id;
  let child_block_id = insert_block(
    &test.document,
    Block {
      parent: block_id.to_string(),
      ty: "text".to_string(),
      data: HashMap::new(),
      external_id: Some(nanoid!(10)),
      external_type: Some(EXTERNAL_TYPE_TEXT.to_string()),
      id: nanoid!(10),
      children: nanoid!(10),
    },
    "".to_string(),
  )
  .unwrap()
  .id;

  let block = move_block(&test.document, &child_block_id, page_id, &block_id);
  let (_page_id, _blocks, children_map) = get_document_data(&test.document);

  assert!(block.is_ok());

  let page_children = &children_map[page_children_id];
  assert_eq!(page_children[0].as_str(), first_child_id);
  assert_eq!(page_children[1].as_str(), block_id);
  assert_eq!(page_children[2].as_str(), child_block_id);
}

#[test]
fn update_block_data_test() {
  let doc_id = "1";
  let test = create_document(1, doc_id);
  let (page_id, blocks, children_map) = get_document_data(&test.document);

  let page_id = page_id.as_str();
  let page_children_id = blocks[page_id].children.as_str();
  let page_children = &children_map[page_children_id];
  let first_child_id = page_children[0].as_str();

  let block = insert_block(
    &test.document,
    Block {
      parent: page_id.to_string(),
      ty: "text".to_string(),
      data: HashMap::new(),
      external_id: None,
      external_type: None,
      id: nanoid!(10),
      children: nanoid!(10),
    },
    first_child_id.to_owned(),
  )
  .unwrap();

  let mut data = HashMap::new();
  data.insert("text".to_string(), to_value("hello").unwrap());
  let res = update_block(&test.document, &block.id, data);
  let (_page_id, blocks, _children_map) = get_document_data(&test.document);

  assert!(res.is_ok());
  let block = &blocks[&block.id];
  assert_eq!(block.data["text"], "hello");
}

#[test]
fn apply_actions_test() {
  let doc_id = "1";
  let test = create_document(1, doc_id);
  let document = &test.document;
  let (page_id, blocks, children_map) = get_document_data(&test.document);
  let first_child_id = &children_map[&blocks[&page_id].children][0];
  let mut data = HashMap::new();
  data.insert("delta".to_string(), json!([]));
  let block = Block {
    id: nanoid!(10),
    ty: "text".to_string(),
    parent: page_id.clone(),
    children: nanoid!(10),
    external_id: None,
    external_type: None,
    data: data.clone(),
  };
  let action_0 = BlockAction {
    action: BlockActionType::Insert,
    payload: BlockActionPayload {
      block: block.clone(),
      prev_id: Some(first_child_id.clone()),
      parent_id: Some(page_id.clone()),
    },
  };
  let action_1 = BlockAction {
    action: BlockActionType::Move,
    payload: BlockActionPayload {
      block: block.clone(),
      prev_id: None,
      parent_id: Some(first_child_id.clone()),
    },
  };
  let actions = vec![action_0, action_1.clone()];
  let actions_json = serde_json::to_value(&actions);
  assert!(actions_json.is_ok());
  apply_actions(document, actions);
  let (page_id, blocks, children_map) = get_document_data(&test.document);
  let page_children = &children_map[&blocks[&page_id].children];
  let first_child_children = &children_map[&blocks[first_child_id].children];
  assert_eq!(page_children.len(), 1);
  assert_eq!(first_child_children.len(), 1);

  let delete_action = BlockAction {
    action: BlockActionType::Delete,
    payload: BlockActionPayload {
      block: block.clone(),
      prev_id: None,
      parent_id: None,
    },
  };

  apply_actions(document, vec![delete_action]);
  let (_, blocks, children_map) = get_document_data(&test.document);
  let first_child_children = &children_map[&blocks[first_child_id].children];
  assert_eq!(first_child_children.len(), 0);
}

#[test]
fn open_document_test() {
  let doc_id = "1";
  let mut test = create_document(1, doc_id);
  let document = &mut test.document;
  let document_data = document.open(|block_events, _| {
    let block_events_json = serde_json::to_value(&block_events);
    assert!(block_events_json.is_ok());
    dbg!(block_events_json.unwrap());
  });
  assert!(document_data.is_ok());
  let document_data = document_data.unwrap();
  let document_data_json = serde_json::to_value(&document_data);
  assert!(document_data_json.is_ok());
  dbg!(document_data_json.unwrap());

  let page_id = document_data.page_id;
  let block = insert_block(
    &test.document,
    Block {
      parent: page_id.clone(),
      ty: "text".to_string(),
      data: HashMap::new(),
      external_id: None,
      external_type: None,
      id: nanoid!(10),
      children: nanoid!(10),
    },
    "".to_string(),
  );

  assert!(block.is_ok());
  let block = block.unwrap();

  let second_block = insert_block(
    &test.document,
    Block {
      parent: page_id,
      ty: "text".to_string(),
      data: HashMap::new(),
      external_id: None,
      external_type: None,
      id: nanoid!(10),
      children: nanoid!(10),
    },
    block.id.clone(),
  );
  assert!(second_block.is_ok());

  let mut data = HashMap::new();
  data.insert("text".to_string(), to_value("hello").unwrap());
  update_block(&test.document, &block.id, data.clone()).unwrap();
  data.insert("text".to_string(), to_value("world").unwrap());
  update_block(&test.document, &block.id, data.clone()).unwrap();
  delete_block(&test.document, &block.id).unwrap();
}
