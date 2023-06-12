use nanoid::nanoid;
use serde_json::to_value;
use std::collections::HashMap;
use std::time::Duration;

use crate::util::{
  create_document, create_document_with_db, db, delete_block, insert_block_for_page,
  open_document_with_db, update_block,
};

const WAIT_TIME: Duration = Duration::from_secs(1);

#[tokio::test]
async fn insert_undo_redo() {
  let doc_id = "1";
  let test = create_document(1, doc_id);
  let document = test.document;
  let block_id = nanoid!(10);

  insert_block_for_page(&document, block_id.clone());

  assert!(document.can_undo());
  assert!(document.undo());

  // there should be no undo action after undo
  assert!(!document.undo());

  // after undo, the block should be deleted
  let insert_block = document.get_block(&block_id);
  assert!(insert_block.is_none());

  assert!(document.can_redo());
  assert!(document.redo());

  // after redo, the block should be restored
  let insert_block = document.get_block(&block_id);
  assert!(insert_block.is_some());

  // there should be no redo action after redo
  assert!(!document.redo());
}

#[tokio::test]
async fn update_undo_redo() {
  let doc_id = "1";
  let test = create_document(1, doc_id);
  let document = test.document;
  let block_id = nanoid!(10);
  insert_block_for_page(&document, block_id.clone());

  // after insert block 1 second, update the block
  tokio::time::sleep(WAIT_TIME).await;
  let mut data = HashMap::new();
  data.insert("text".to_string(), to_value("hello").unwrap());
  update_block(&document, &block_id, data.clone()).unwrap();

  assert!(document.can_undo());
  assert!(document.undo());

  // after undo, the data of block should be default
  let block = document.get_block(&block_id).unwrap();
  assert_eq!(block.data, Default::default());

  assert!(document.can_redo());
  assert!(document.redo());

  // after redo, the data of block should be updated
  let block = document.get_block(&block_id).unwrap();
  assert_eq!(block.data, data);
}

#[tokio::test]
async fn delete_undo_redo() {
  let doc_id = "1";
  let test = create_document(1, doc_id);
  let document = test.document;
  let block_id = nanoid!(10);
  insert_block_for_page(&document, block_id.clone());

  // after insert block 1 second, delete the block
  tokio::time::sleep(WAIT_TIME).await;
  delete_block(&document, &block_id).unwrap();

  assert!(document.can_undo());
  assert!(document.undo());

  // after undo, the block should be restored
  let block = document.get_block(&block_id);
  assert!(block.is_some());

  assert!(document.can_redo());
  assert!(document.redo());

  // after redo, the block should be deleted
  let block = document.get_block(&block_id);
  assert!(block.is_none());
}

#[tokio::test]
async fn mutilple_undo_redo_test() {
  let doc_id = "1";
  let test = create_document(1, doc_id);
  let document = test.document;

  let block_id = nanoid!(10);
  insert_block_for_page(&document, block_id.clone());

  // after insert block 1 second, update the block
  tokio::time::sleep(WAIT_TIME).await;
  let mut data = HashMap::new();
  data.insert("text".to_string(), to_value("hello").unwrap());
  update_block(&document, &block_id, data.clone()).unwrap();

  // after insert block 1 second, delete the block
  tokio::time::sleep(WAIT_TIME).await;
  delete_block(&document, &block_id).unwrap();

  assert!(document.can_undo());
  assert!(document.undo());
  // after first undo, action1: revert delete block
  let block = document.get_block(&block_id).unwrap();
  assert_eq!(block.data, data);

  assert!(document.can_undo());
  assert!(document.undo());
  // after second undo, action2: revert update block
  let block = document.get_block(&block_id).unwrap();
  assert_eq!(block.data, Default::default());

  assert!(document.can_undo());
  assert!(document.undo());
  // after third undo, action3: revert insert block
  let block = document.get_block(&block_id);
  assert!(block.is_none());
  assert!(!document.can_undo());

  assert!(document.can_redo());
  assert!(document.redo());
  // after first redo, revert action3, insert block
  let block = document.get_block(&block_id).unwrap();
  assert_eq!(block.data, Default::default());

  assert!(document.can_redo());
  assert!(document.redo());
  // after second redo, revert action2, update block
  let block = document.get_block(&block_id).unwrap();
  assert_eq!(block.data, data);

  assert!(document.can_redo());
  assert!(document.redo());
  // after third redo, revert action1, delete block
  let block = document.get_block(&block_id);
  assert!(block.is_none());

  assert!(!document.can_redo());
}

#[tokio::test]
async fn undo_redo_after_reopen_document() {
  let doc_id = "1";
  let db = db();
  let test = create_document_with_db(1, doc_id, db.clone());
  let document = test.document;
  // after create document, can't undo
  assert!(!document.can_undo());

  // after insert block, can undo
  let block_id = nanoid!(10);
  insert_block_for_page(&document, block_id.clone());
  assert!(document.can_undo());

  // close document
  drop(document);

  // reopen document, can't undo
  let test = open_document_with_db(1, doc_id, db);
  let document = test.document;
  assert!(!document.can_undo());

  // update block, can undo
  let mut data = HashMap::new();
  data.insert("text".to_string(), to_value("hello").unwrap());
  update_block(&document, &block_id, data.clone()).unwrap();
  assert!(document.can_undo());

  // There is no undo action, so can't redo
  assert!(!document.can_redo());

  // after undo, the data of block should be default
  assert!(document.undo());
  let block = document.get_block(&block_id).unwrap();
  assert_eq!(block.data, Default::default());

  // There has undo action, so can redo
  assert!(document.can_redo());
  assert!(document.redo());
  // after redo, the data of block should be updated
  let block = document.get_block(&block_id).unwrap();
  assert_eq!(block.data, data);
}
