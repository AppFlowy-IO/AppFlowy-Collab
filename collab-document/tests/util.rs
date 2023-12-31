#![allow(clippy::type_complexity)]

use std::collections::HashMap;
use std::fs::{create_dir_all, File};
use std::io::copy;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Once};

use collab::preclude::CollabBuilder;
use collab_document::blocks::{Block, BlockAction, DocumentData, DocumentMeta};
use collab_document::document::Document;
use collab_document::error::DocumentError;
use collab_persistence::kv_impls::rocks_kv::RocksCollabDB;
use collab_plugins::local_storage::rocksdb_plugin::RocksdbDiskPlugin;
use nanoid::nanoid;
use serde_json::{json, Value};
use tempfile::TempDir;
use tracing_subscriber::{fmt::Subscriber, util::SubscriberInitExt, EnvFilter};
use zip::ZipArchive;

pub struct DocumentTest {
  pub document: Document,
  pub db: Arc<RocksCollabDB>,
}

impl DocumentTest {
  pub async fn new(uid: i64, doc_id: &str) -> Self {
    let db = document_storage();
    Self::new_with_db(uid, doc_id, db).await
  }

  pub async fn new_with_db(uid: i64, doc_id: &str, db: Arc<RocksCollabDB>) -> Self {
    let disk_plugin = RocksdbDiskPlugin::new(uid, Arc::downgrade(&db), None);
    let collab = CollabBuilder::new(1, doc_id)
      .with_plugin(disk_plugin)
      .with_device_id("1")
      .build()
      .unwrap();
    collab.lock().initialize();

    let mut blocks = HashMap::new();
    let mut children_map = HashMap::new();
    let mut text_map = HashMap::new();

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
    let first_text_external_id = nanoid!(10);
    let empty_text_delta = "[]".to_string();
    text_map.insert(first_text_external_id.clone(), empty_text_delta);
    blocks.insert(
      first_text_id.clone(),
      Block {
        id: first_text_id,
        ty: "text".to_string(),
        parent: page_id.clone(),
        children: first_text_children_id,
        data: data.clone(),
        external_id: Some(first_text_external_id),
        external_type: Some("text".to_string()),
      },
    );
    let meta = DocumentMeta {
      children_map,
      text_map: Some(text_map),
    };
    let document_data = DocumentData {
      page_id,
      blocks,
      meta,
    };
    let document = Document::create_with_data(Arc::new(collab), document_data).unwrap();
    Self { document, db }
  }
}

impl Deref for DocumentTest {
  type Target = Document;

  fn deref(&self) -> &Self::Target {
    &self.document
  }
}

pub async fn open_document_with_db(uid: i64, doc_id: &str, db: Arc<RocksCollabDB>) -> Document {
  setup_log();
  let disk_plugin = RocksdbDiskPlugin::new(uid, Arc::downgrade(&db), None);
  let collab = CollabBuilder::new(uid, doc_id)
    .with_plugin(disk_plugin)
    .with_device_id("1")
    .build()
    .unwrap();
  collab.lock().initialize();

  Document::open(Arc::new(collab)).unwrap()
}

pub fn document_storage() -> Arc<RocksCollabDB> {
  let tempdir = TempDir::new().unwrap();
  let path = tempdir.into_path();
  Arc::new(RocksCollabDB::open_opt(path, false).unwrap())
}

fn setup_log() {
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
  let document_data = document.get_document_data().unwrap();

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

pub fn apply_actions(document: &Document, actions: Vec<BlockAction>) {
  document.apply_action(actions)
}

pub fn insert_block_for_page(document: &Document, block_id: String) -> Block {
  let (page_id, _, _) = get_document_data(document);
  let block = Block {
    id: block_id,
    ty: "paragraph".to_string(),
    parent: page_id,
    children: "".to_string(),
    external_id: None,
    external_type: None,
    data: Default::default(),
  };

  insert_block(document, block, "".to_string()).unwrap()
}

pub struct Cleaner(PathBuf);

impl Cleaner {
  pub fn new(dir: PathBuf) -> Self {
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

pub fn unzip_history_document_db(folder_name: &str) -> std::io::Result<(Cleaner, PathBuf)> {
  // Open the zip file
  let zip_file_path = format!("./tests/history_document/{}.zip", folder_name);
  let reader = File::open(zip_file_path)?;
  let output_folder_path = format!("./tests/history_document/unit_test_{}", nanoid!(6));

  // Create a ZipArchive from the file
  let mut archive = ZipArchive::new(reader)?;

  // Iterate through each file in the zip
  for i in 0..archive.len() {
    let mut file = archive.by_index(i)?;
    let outpath = Path::new(&output_folder_path).join(file.mangled_name());

    if file.name().ends_with('/') {
      // Create directory
      create_dir_all(&outpath)?;
    } else {
      // Write file
      if let Some(p) = outpath.parent() {
        if !p.exists() {
          create_dir_all(p)?;
        }
      }
      let mut outfile = File::create(&outpath)?;
      copy(&mut file, &mut outfile)?;
    }
  }
  let path = format!("{}/{}", output_folder_path, folder_name);
  Ok((
    Cleaner::new(PathBuf::from(output_folder_path)),
    PathBuf::from(path),
  ))
}
