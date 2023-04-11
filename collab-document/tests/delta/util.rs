use collab::plugin_impl::disk::CollabDiskPlugin;
use collab::preclude::CollabBuilder;
use std::collections::HashMap;

use collab_document::blocks::{Block, BlockAction, DocumentData, DocumentMeta};
use collab_document::document::Document;
use collab_document::error::DocumentError;
use collab_persistence::CollabKV;
use nanoid::nanoid;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use tempfile::TempDir;

pub struct DocumentTest {
  pub document: Document,
  #[allow(dead_code)]
  cleaner: Cleaner,
}

pub fn create_document(doc_id: &str) -> DocumentTest {
  let uid = 1;
  let tempdir = TempDir::new().unwrap();
  let path = tempdir.into_path();
  let db = Arc::new(CollabKV::open(path.clone()).unwrap());
  let disk_plugin = CollabDiskPlugin::new(uid, db).unwrap();
  let cleaner = Cleaner::new(path);

  let collab = CollabBuilder::new(1, doc_id)
    .with_plugin(disk_plugin)
    .build();
  collab.initial();

  let mut blocks = HashMap::new();
  let mut children_map = HashMap::new();

  let mut data = HashMap::new();
  data.insert("delta".to_string(), json!([]));
  let page_id = nanoid!(10);
  let page_children_id = nanoid!(10);
  blocks.insert(
    page_id.clone(),
    Block {
      id: page_id.clone(),
      ty: "page".to_string(),
      parent: "".to_string(),
      children: page_children_id.clone(),
      data: data.clone(),
      external_id: None,
      external_type: None,
    },
  );

  let first_text_id = nanoid!(10);
  children_map.insert(page_children_id.clone(), vec![first_text_id.clone()]);
  let first_text_children_id = nanoid!(10);
  children_map.insert(first_text_children_id.clone(), vec![]);
  blocks.insert(
    first_text_id.clone(),
    Block {
      id: first_text_id.clone(),
      ty: "text".to_string(),
      parent: page_id.clone(),
      children: first_text_children_id,
      data: data.clone(),
      external_id: None,
      external_type: None,
    },
  );
  let meta = DocumentMeta { children_map };
  let document_data = DocumentData {
    page_id,
    blocks,
    meta,
  };
  match Document::create(collab, document_data) {
    Ok(document) => DocumentTest { document, cleaner },
    Err(e) => panic!("create document error: {}", e),
  }
}

pub fn insert_block(
  document: &Document,
  block: Block,
  prev_id: String,
) -> Result<Block, DocumentError> {
  document.with_transact_mut(|txn| document.insert_block(txn, block, Some(prev_id)))
}

pub fn get_document_data(
  document: &Document,
) -> (
  String,
  Rc<HashMap<String, Block>>,
  Rc<HashMap<String, Vec<String>>>,
) {
  let document_data = document.get_document().unwrap();

  let page_id = document_data.page_id.as_str();
  let blocks = Rc::new(document_data.blocks);
  let meta = document_data.meta;
  let children_map = Rc::new(meta.children_map);

  return (page_id.to_owned(), blocks, children_map);
}

pub fn delete_block(document: &Document, block_id: &str) -> Result<(), DocumentError> {
  document.with_transact_mut(|txn| document.delete_block(txn, block_id))
}

pub fn update_block(
  document: &Document,
  block_id: &str,
  data: HashMap<String, Value>,
) -> Result<(), DocumentError> {
  document.with_transact_mut(|txn| document.update_block_data(txn, block_id, data))
}

pub fn move_block(
  document: &Document,
  block_id: &str,
  parent_id: &str,
  prev_id: &str,
) -> Result<(), DocumentError> {
  document.with_transact_mut(|txn| {
    document.move_block(
      txn,
      block_id,
      Some(parent_id.to_owned()),
      Some(prev_id.to_owned()),
    )
  })
}

pub fn apply_actions(document: &Document, actions: Vec<BlockAction>) {
  document.apply_action(actions)
}

struct Cleaner(PathBuf);

impl Cleaner {
  fn new(dir: PathBuf) -> Self {
    Cleaner(dir)
  }

  fn cleanup(dir: &PathBuf) {
    let _ = std::fs::remove_dir_all(dir);
  }
}

impl Drop for Cleaner {
  fn drop(&mut self) {
    Self::cleanup(&self.0)
  }
}
