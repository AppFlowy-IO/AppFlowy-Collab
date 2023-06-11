use collab_document::blocks::Block;
use nanoid::nanoid;
use serde_json::to_value;
use std::collections::HashMap;
use std::time::Duration;

use crate::util::{create_document, delete_block, get_document_data, insert_block, update_block};

#[tokio::test]
async fn insert_undo_redo() {
  let doc_id = "1";
  let test = create_document(1, doc_id);

  tokio::time::sleep(Duration::from_secs(1)).await;

  let (page_id, _, _) = get_document_data(&test.document);
  let block_id = nanoid!(10);
  let block = Block {
    id: block_id.clone(),
    ty: "paragraph".to_string(),
    parent: page_id.to_string(), // empty parent id
    children: "".to_string(),
    external_id: None,
    external_type: None,
    data: Default::default(),
  };

  insert_block(&test.document, block.clone(), "".to_string()).unwrap();

  let can_undo = test.document.can_undo();
  assert!(can_undo);
  let undo = &test.document.undo();
  assert_eq!(undo.to_owned(), true);
  let insert_block = &test.document.get_block(&block_id);
  assert_eq!(insert_block.is_none(), true);

  let can_redo = test.document.can_redo();
  assert!(can_redo);
  let redo = &test.document.redo();
  assert_eq!(redo.to_owned(), true);
  let insert_block = &test.document.get_block(&block_id);
  assert_eq!(insert_block.is_none(), false);
}

#[tokio::test]
async fn update_undo_redo() {
  let doc_id = "1";
  let test = create_document(1, doc_id);

  let (page_id, _, _) = get_document_data(&test.document);
  let block_id = nanoid!(10);
  let block = Block {
    id: block_id.clone(),
    ty: "paragraph".to_string(),
    parent: page_id.to_string(), // empty parent id
    children: "".to_string(),
    external_id: None,
    external_type: None,
    data: Default::default(),
  };
  insert_block(&test.document, block.clone(), "".to_string()).unwrap();

  tokio::time::sleep(Duration::from_secs(1)).await;

  let block = test.document.get_block(&block_id).unwrap();
  let mut data = HashMap::new();
  data.insert("text".to_string(), to_value("hello").unwrap());
  update_block(&test.document, &block.id, data.clone()).unwrap();

  let can_undo = test.document.can_undo();
  assert!(can_undo);
  let undo = &test.document.undo();
  assert_eq!(undo.to_owned(), true);
  let block = &test.document.get_block(&block_id).unwrap();
  assert_eq!(block.data, Default::default());

  let can_redo = test.document.can_redo();
  assert!(can_redo);
  let redo = &test.document.redo();
  assert_eq!(redo.to_owned(), true);
  let block = &test.document.get_block(&block_id).unwrap();
  assert_eq!(block.data, data);
}

#[tokio::test]
async fn delete_undo_redo() {
  let doc_id = "1";
  let test = create_document(1, doc_id);

  let (page_id, _, _) = get_document_data(&test.document);
  let block_id = nanoid!(10);
  let block = Block {
    id: block_id.clone(),
    ty: "paragraph".to_string(),
    parent: page_id.to_string(), // empty parent id
    children: "".to_string(),
    external_id: None,
    external_type: None,
    data: Default::default(),
  };
  insert_block(&test.document, block.clone(), "".to_string()).unwrap();

  tokio::time::sleep(Duration::from_secs(1)).await;

  delete_block(&test.document, &block_id).unwrap();

  let can_undo = test.document.can_undo();
  assert!(can_undo);
  let undo = &test.document.undo();
  assert_eq!(undo.to_owned(), true);
  let block = &test.document.get_block(&block_id);
  assert_eq!(block.is_none(), false);

  let can_redo = test.document.can_redo();
  assert!(can_redo);
  let redo = &test.document.redo();
  assert_eq!(redo.to_owned(), true);
  let block = &test.document.get_block(&block_id);
  assert_eq!(block.is_none(), true);
}

#[tokio::test]
async fn mutilple_undo_redo_test() {
  let doc_id = "1";
  let test = create_document(1, doc_id);

  tokio::time::sleep(Duration::from_secs(1)).await;

  let (page_id, _, _) = get_document_data(&test.document);
  let block_id = nanoid!(10);
  let block = Block {
    id: block_id.clone(),
    ty: "paragraph".to_string(),
    parent: page_id.to_string(), // empty parent id
    children: "".to_string(),
    external_id: None,
    external_type: None,
    data: Default::default(),
  };

  insert_block(&test.document, block.clone(), "".to_string()).unwrap();

  tokio::time::sleep(Duration::from_secs(1)).await;

  let block = test.document.get_block(&block_id).unwrap();
  let mut data = HashMap::new();
  data.insert("text".to_string(), to_value("hello").unwrap());
  update_block(&test.document, &block.id, data.clone()).unwrap();

  tokio::time::sleep(Duration::from_secs(1)).await;

  delete_block(&test.document, &block_id).unwrap();

  tokio::time::sleep(Duration::from_secs(1)).await;

  let can_undo = test.document.can_undo();
  assert!(can_undo);
  let _ = &test.document.undo();
  let block = &test.document.get_block(&block_id).unwrap();
  assert_eq!(block.data, data);
  let _ = &test.document.undo();
  let block = &test.document.get_block(&block_id).unwrap();
  assert_eq!(block.data, Default::default());
  let _ = &test.document.undo();
  let block = &test.document.get_block(&block_id);
  assert_eq!(block.is_none(), true);

  let can_redo = test.document.can_redo();
  assert!(can_redo);
  let _ = &test.document.redo();
  let block = &test.document.get_block(&block_id).unwrap();
  assert_eq!(block.data, Default::default());
  let _ = &test.document.redo();
  let block = &test.document.get_block(&block_id).unwrap();
  assert_eq!(block.data, data);
  let _ = &test.document.redo();
  let block = &test.document.get_block(&block_id);
  assert_eq!(block.is_none(), true);
  let can_redo = test.document.can_redo();
  assert_eq!(can_redo, false);
}
