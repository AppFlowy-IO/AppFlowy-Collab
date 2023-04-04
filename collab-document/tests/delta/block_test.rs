use crate::util::{
  create_document, delete_block, get_document_data, insert_block, move_block, update_block,
};
use collab_document::blocks::EXTERNAL_TYPE_TEXT;
use collab_document::document::InsertBlockArgs;
use nanoid::nanoid;
use serde_json::to_value;
use std::collections::HashMap;

#[test]
fn create_block_test() {
  let doc_id = "1";
  let test = create_document(doc_id);
  let (page_id, blocks, text_map, children_map) = get_document_data(&test.document);

  let page_id = page_id.as_str();
  assert!(blocks[page_id].is_object());
  assert!(blocks[page_id]["ty"] == "page");
  assert!(blocks[page_id]["data"].is_object());
  assert!(blocks[page_id]["external_id"].is_string());

  let page_external_id = blocks[page_id]["external_id"].as_str().unwrap();
  let page_children_id = blocks[page_id]["children"].as_str().unwrap();
  assert!(blocks[page_id]["external_type"] == EXTERNAL_TYPE_TEXT);
  assert!(blocks[page_id]["children"].is_string());
  assert!(blocks[page_id]["parent"] == "");
  assert!(text_map[page_external_id].is_array());
  assert!(children_map[page_children_id].is_array());

  let page_children = children_map[page_children_id].as_array().unwrap();
  assert_eq!(page_children.len(), 1);
  let first_child_id = page_children[0].as_str().unwrap();
  assert!(blocks[first_child_id].is_object());
  assert_eq!(blocks[first_child_id]["parent"], page_id.to_string());
  assert!(blocks[first_child_id]["external_type"] == EXTERNAL_TYPE_TEXT);
}

#[test]
fn insert_block_test() {
  let doc_id = "1";
  let test = create_document(doc_id);

  let (page_id, blocks, _, children_map) = get_document_data(&test.document);

  let page_id = page_id.as_str();
  let page_children_id = blocks[page_id]["children"].as_str().unwrap();
  let page_children = children_map[page_children_id].as_array().unwrap();
  let first_child_id = page_children[0].as_str().unwrap();

  let block_external_id = nanoid!(10);
  let block_children_id = nanoid!(10);
  let block_id = nanoid!(10);
  let block = insert_block(
    &test.document,
    InsertBlockArgs {
      parent_id: page_id.to_string(),
      ty: "text".to_string(),
      data: HashMap::new(),
      external_id: block_external_id.to_string(),
      external_type: EXTERNAL_TYPE_TEXT.to_string(),
      block_id: block_id.to_string(),
      children_id: block_children_id.to_string(),
    },
    first_child_id,
  );
  let block_child = insert_block(
    &test.document,
    InsertBlockArgs {
      parent_id: block_id.to_string(),
      ty: "text".to_string(),
      data: HashMap::new(),
      external_id: nanoid!(10).to_string(),
      external_type: EXTERNAL_TYPE_TEXT.to_string(),
      block_id: nanoid!(10).to_string(),
      children_id: nanoid!(10).to_string(),
    },
    "",
  );
  assert!(block_child.is_ok());
  assert!(block.is_ok());
  let block = block.unwrap();
  let (page_id, blocks, text_map, children_map) = get_document_data(&test.document);
  assert!(blocks[block.id].is_object());
  assert_eq!(block.parent, page_id.to_string());
  assert_eq!(block.children, block_children_id);
  assert_eq!(block.external_id, block_external_id);
  assert_eq!(block.external_type, EXTERNAL_TYPE_TEXT);
  assert_eq!(block.ty, "text");
  assert!(children_map[&block_children_id].is_array());
  assert_eq!(
    children_map[&block_children_id].as_array().unwrap().len(),
    1
  );
  assert_eq!(
    children_map[&block_children_id].as_array().unwrap()[0]
      .as_str()
      .unwrap()
      .to_string(),
    block_child.unwrap().id
  );
  assert!(text_map[block_external_id].is_array());
  let page_children = children_map[page_children_id].as_array().unwrap();

  assert_eq!(page_children.len(), 2);
  assert!(page_children[1].as_str().unwrap().to_string() == block_id);
}

#[test]
fn delete_block_test() {
  let doc_id = "1";
  let test = create_document(doc_id);

  let (page_id, blocks, _, children_map) = get_document_data(&test.document);

  let page_id = page_id.as_str();
  let page_children_id = blocks[page_id]["children"].as_str().unwrap();
  let page_children = children_map[page_children_id].as_array().unwrap();
  let first_child_id = page_children[0].as_str().unwrap();

  let block_external_id = nanoid!(10);
  let block_children_id = nanoid!(10);
  let block_id = nanoid!(10);
  let block = insert_block(
    &test.document,
    InsertBlockArgs {
      parent_id: page_id.to_string(),
      ty: "text".to_string(),
      data: HashMap::new(),
      external_id: block_external_id.to_string(),
      external_type: EXTERNAL_TYPE_TEXT.to_string(),
      block_id: block_id.to_string(),
      children_id: block_children_id.to_string(),
    },
    first_child_id,
  );
  insert_block(
    &test.document,
    InsertBlockArgs {
      parent_id: block_id.to_string(),
      ty: "text".to_string(),
      data: HashMap::new(),
      external_id: nanoid!(10).to_string(),
      external_type: EXTERNAL_TYPE_TEXT.to_string(),
      block_id: nanoid!(10).to_string(),
      children_id: nanoid!(10).to_string(),
    },
    "",
  )
  .unwrap();
  assert!(block.is_ok());
  let block = delete_block(&test.document, &block.unwrap().id);
  let block = block.unwrap();
  let (_, blocks, text_map, children_map) = get_document_data(&test.document);
  assert!(blocks[block.id].is_null());
  assert!(children_map[block_children_id].is_null());
  assert!(text_map[block_external_id].is_null());
  let page_children = children_map[page_children_id].as_array().unwrap();
  assert_eq!(page_children.len(), 1);
  assert_ne!(page_children[0].as_str().unwrap().to_string(), block_id);
  assert_eq!(
    page_children[0].as_str().unwrap().to_string(),
    first_child_id
  );
}

#[test]
fn move_block_test() {
  let doc_id = "1";
  let test = create_document(doc_id);
  let (page_id, blocks, _, children_map) = get_document_data(&test.document);

  let page_id = page_id.as_str();
  let page_children_id = blocks[page_id]["children"].as_str().unwrap();
  let page_children = children_map[page_children_id].as_array().unwrap();
  let first_child_id = page_children[0].as_str().unwrap();

  let block = insert_block(
    &test.document,
    InsertBlockArgs {
      parent_id: page_id.to_string(),
      ty: "text".to_string(),
      data: HashMap::new(),
      external_id: nanoid!(10).to_string(),
      external_type: EXTERNAL_TYPE_TEXT.to_string(),
      block_id: nanoid!(10).to_string(),
      children_id: nanoid!(10).to_string(),
    },
    first_child_id,
  )
  .unwrap();

  let block_id = block.id;
  let child_block_id = insert_block(
    &test.document,
    InsertBlockArgs {
      parent_id: block_id.to_string(),
      ty: "text".to_string(),
      data: HashMap::new(),
      external_id: nanoid!(10).to_string(),
      external_type: EXTERNAL_TYPE_TEXT.to_string(),
      block_id: nanoid!(10).to_string(),
      children_id: nanoid!(10).to_string(),
    },
    "",
  )
  .unwrap()
  .id;

  let block = move_block(&test.document, &child_block_id, page_id, &block_id);

  assert!(block.is_ok());

  let (_, _, _, children_map) = get_document_data(&test.document);
  let page_children = children_map[page_children_id].as_array().unwrap();
  assert!(page_children[0].as_str().unwrap().to_string() == first_child_id);
  assert!(page_children[1].as_str().unwrap().to_string() == block_id);
  assert!(page_children[2].as_str().unwrap().to_string() == child_block_id);
}

#[test]
fn update_block_data_test() {
  let doc_id = "1";
  let test = create_document(doc_id);
  let (page_id, blocks, _, children_map) = get_document_data(&test.document);

  let page_id = page_id.as_str();
  let page_children_id = blocks[page_id]["children"].as_str().unwrap();
  let page_children = children_map[page_children_id].as_array().unwrap();
  let first_child_id = page_children[0].as_str().unwrap();

  let block = insert_block(
    &test.document,
    InsertBlockArgs {
      parent_id: page_id.to_string(),
      ty: "text".to_string(),
      data: HashMap::new(),
      external_id: nanoid!(10).to_string(),
      external_type: EXTERNAL_TYPE_TEXT.to_string(),
      block_id: nanoid!(10).to_string(),
      children_id: nanoid!(10).to_string(),
    },
    first_child_id,
  )
  .unwrap();

  let mut data = HashMap::new();
  data.insert("text".to_string(), to_value("hello").unwrap());
  let res = update_block(&test.document, &block.id, data);
  assert!(res.is_ok());
  let (_, blocks, _, _) = get_document_data(&test.document);
  let block = blocks[block.id].as_object().unwrap();
  assert_eq!(block["data"]["text"].as_str().unwrap(), "hello");
}
