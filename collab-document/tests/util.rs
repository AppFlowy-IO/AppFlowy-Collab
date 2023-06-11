#![allow(clippy::type_complexity)]

use std::collections::HashMap;
use std::ops::Deref;
use std::sync::{Arc, Once};

use collab::preclude::CollabBuilder;
use collab_document::blocks::{Block, BlockAction, DocumentData, DocumentMeta};
use collab_document::document::Document;
use collab_document::error::DocumentError;
use collab_persistence::kv::rocks_kv::RocksCollabDB;
use nanoid::nanoid;
use serde_json::{json, Value};

use collab_plugins::disk::rocksdb::RocksdbDiskPlugin;
use tempfile::TempDir;
use tracing_subscriber::{fmt::Subscriber, util::SubscriberInitExt, EnvFilter};

pub struct DocumentTest {
  pub document: Document,
  pub db: Arc<RocksCollabDB>,
}

impl Deref for DocumentTest {
  type Target = Document;

  fn deref(&self) -> &Self::Target {
    &self.document
  }
}

pub fn create_document(uid: i64, doc_id: &str) -> DocumentTest {
  let db = db();
  create_document_with_db(uid, doc_id, db)
}

pub fn create_document_with_db(uid: i64, doc_id: &str, db: Arc<RocksCollabDB>) -> DocumentTest {
  let disk_plugin = RocksdbDiskPlugin::new(uid, db.clone());
  let collab = CollabBuilder::new(1, doc_id)
    .with_plugin(disk_plugin)
    .build();
  collab.lock().initialize();

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
  children_map.insert(page_children_id, vec![first_text_id.clone()]);
  let first_text_children_id = nanoid!(10);
  children_map.insert(first_text_children_id.clone(), vec![]);
  blocks.insert(
    first_text_id.clone(),
    Block {
      id: first_text_id,
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

  match Document::create_with_data(Arc::new(collab), document_data) {
    Ok(document) => DocumentTest { document, db },
    Err(e) => panic!("create document error: {}", e),
  }
}

pub fn open_document_with_db(uid: i64, doc_id: &str, db: Arc<RocksCollabDB>) -> DocumentTest {
  let disk_plugin = RocksdbDiskPlugin::new(uid, db.clone());
  let collab = CollabBuilder::new(uid, doc_id)
    .with_plugin(disk_plugin)
    .build();
  collab.initial();

  DocumentTest {
    document: Document::create(Arc::new(collab)).unwrap(),
    db,
  }
}

pub fn db() -> Arc<RocksCollabDB> {
  static START: Once = Once::new();
  START.call_once(|| {
    let level = "info";
    let mut filters = vec![];
    filters.push(format!("collab_persistence={}", level));
    filters.push(format!("collab={}", level));
    filters.push(format!("collab_database={}", level));
    std::env::set_var("RUST_LOG", filters.join(","));

    let subscriber = Subscriber::builder()
      .with_env_filter(EnvFilter::from_default_env())
      .with_ansi(true)
      .finish();
    subscriber.try_init().unwrap();
  });

  let tempdir = TempDir::new().unwrap();
  let path = tempdir.into_path();
  Arc::new(RocksCollabDB::open(path).unwrap())
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
) -> (String, HashMap<String, Block>, HashMap<String, Vec<String>>) {
  let document_data = document.get_document().unwrap();

  let page_id = document_data.page_id.clone();
  let blocks = document_data.blocks;
  let meta = document_data.meta;
  let children_map = meta.children_map;

  (page_id, blocks, children_map)
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

pub fn insert_block_for_page(document: &Document, block_id: String) {
  let (page_id, _, _) = get_document_data(document);
  let block = Block {
    id: block_id.clone(),
    ty: "paragraph".to_string(),
    parent: page_id.to_string(), // empty parent id
    children: "".to_string(),
    external_id: None,
    external_type: None,
    data: Default::default(),
  };

  insert_block(document, block.clone(), "".to_string()).unwrap();
}

pub fn update_block_with_data(block_id: &str, document: &Document, data: HashMap<String, Value>) {
  let block = document.get_block(block_id).unwrap();
  update_block(document, &block.id, data.clone()).unwrap();
}

// struct Cleaner(PathBuf);
//
// impl Cleaner {
//   fn new(dir: PathBuf) -> Self {
//     Cleaner(dir)
//   }
//
//   fn cleanup(dir: &PathBuf) {
//     let _ = std::fs::remove_dir_all(dir);
//   }
// }
//
// impl Drop for Cleaner {
//   fn drop(&mut self) {
//     Self::cleanup(&self.0)
//   }
// }
