use collab::plugin_impl::disk::CollabDiskPlugin;
use collab::preclude::{CollabBuilder, JsonValue};
use std::collections::HashMap;

use collab_document::document::{Document, InsertBlockArgs};
use collab_persistence::CollabKV;
use nanoid::nanoid;
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

  let document = Document::create(collab);
  DocumentTest { document, cleaner }
}

pub fn insert_block(document: &Document, ty: String, parent_id: &str, prev_id: &str) -> String {
  let block_id = nanoid!();
  let mut data = HashMap::new();
  data.insert("text".to_string(), JsonValue::String(nanoid!()));
  document.with_txn(|txn| {
    document
      .insert_block(
        txn,
        InsertBlockArgs {
          parent_id: parent_id.to_string(),
          block_id: block_id.clone(),
          data,
          children_id: nanoid!(),
          ty,
        },
        prev_id.to_string(),
      )
      .unwrap();
  });
  block_id
}

pub fn delete_block(document: &Document, block_id: &str) {
  document.with_txn(|txn| {
    document.delete_block(txn, block_id);
  });
}

pub fn move_block(document: &Document, block_id: &str, parent_id: &str, prev_id: &str) {
  document.with_txn(|txn| {
    document.move_block(txn, block_id, parent_id, prev_id);
  });
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
