use collab::plugin_impl::disk::CollabDiskPlugin;
use collab::preclude::CollabBuilder;

use collab_document::blocks::Block;
use collab_document::document::{Document, InsertBlockArgs};
use collab_document::error::DocumentError;
use collab_persistence::CollabKV;
use serde_json::Value;
use std::path::PathBuf;
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

  match Document::create(collab) {
    Ok(document) => DocumentTest { document, cleaner },
    Err(e) => panic!("create document error: {}", e),
  }
}

pub fn insert_block(
  document: &Document,
  block: InsertBlockArgs,
  prev_id: &str,
) -> Result<Block, DocumentError> {
  document
    .root
    .with_transact_mut(|txn| document.insert_block(txn, block, prev_id.to_string()))
}

pub fn get_document_data(document: &Document) -> (String, Value, Value, Value) {
  let document_data = document.get_document().unwrap();
  let document = &document_data["document"];

  let page_id = document["page_id"].as_str().unwrap();
  let blocks = &document["blocks"];
  let meta = &document["meta"];
  let text_map = &meta["text_map"];
  let children_map = &meta["children_map"];

  return (
    page_id.to_owned(),
    blocks.clone(),
    text_map.clone(),
    children_map.clone(),
  );
}

pub fn delete_block(document: &Document, block_id: &str) -> Result<Block, DocumentError> {
  document
    .root
    .with_transact_mut(|txn| document.delete_block(txn, block_id))
}

pub fn move_block(
  document: &Document,
  block_id: &str,
  parent_id: &str,
  prev_id: &str,
) -> Result<(), DocumentError> {
  document
    .root
    .with_transact_mut(|txn| document.move_block(txn, block_id, parent_id, prev_id))
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
