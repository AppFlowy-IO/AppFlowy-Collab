#![allow(clippy::type_complexity)]

use std::collections::HashMap;
use std::fs::{File, create_dir_all};
use std::io::copy;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Once};

use collab::core::collab::{CollabOptions, default_client_id};
use collab::core::origin::{CollabClient, CollabOrigin};
use collab::preclude::Collab;
use collab_document::blocks::{Block, BlockAction, DocumentData, DocumentMeta};
use collab_document::document::Document;
use collab_entity::CollabType;
use collab_plugins::CollabKVDB;
use collab_plugins::local_storage::rocksdb::rocksdb_plugin::RocksdbDiskPlugin;
use collab_plugins::local_storage::rocksdb::util::KVDBCollabPersistenceImpl;
use nanoid::nanoid;
use serde_json::json;
use tempfile::TempDir;
use tracing_subscriber::{EnvFilter, fmt::Subscriber, util::SubscriberInitExt};
use uuid::Uuid;
use zip::ZipArchive;

pub struct DocumentTest {
  pub workspace_id: String,
  pub document: Document,
  pub db: Arc<CollabKVDB>,
}

impl DocumentTest {
  pub fn new(uid: i64, doc_id: &str) -> Self {
    let workspace_id = Uuid::new_v4().to_string();
    let db = document_storage();
    Self::new_with_db(uid, workspace_id, doc_id, db)
  }

  pub fn new_with_db(uid: i64, workspace_id: String, doc_id: &str, db: Arc<CollabKVDB>) -> Self {
    let disk_plugin = RocksdbDiskPlugin::new(
      uid,
      workspace_id.clone(),
      doc_id.to_string(),
      CollabType::Document,
      Arc::downgrade(&db),
    );
    let data_source = KVDBCollabPersistenceImpl {
      db: Arc::downgrade(&db),
      uid,
      workspace_id: workspace_id.clone(),
    };

    let options = CollabOptions::new(doc_id.to_string(), default_client_id())
      .with_data_source(data_source.into());
    let client = CollabClient::new(uid, "1");
    let collab = Collab::new_with_options(CollabOrigin::Client(client), options).unwrap();
    collab.add_plugin(Box::new(disk_plugin));

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
    let mut document = Document::create_with_data(collab, document_data).unwrap();
    document.initialize();
    Self {
      workspace_id,
      document,
      db,
    }
  }
}

impl Deref for DocumentTest {
  type Target = Document;

  fn deref(&self) -> &Self::Target {
    &self.document
  }
}

pub fn open_document_with_db(
  uid: i64,
  workspace_id: &str,
  doc_id: &str,
  db: Arc<CollabKVDB>,
) -> Document {
  setup_log();
  let disk_plugin = RocksdbDiskPlugin::new(
    uid,
    workspace_id.to_string(),
    doc_id.to_string(),
    CollabType::Document,
    Arc::downgrade(&db),
  );
  let data_source = KVDBCollabPersistenceImpl {
    db: Arc::downgrade(&db),
    uid,
    workspace_id: workspace_id.to_string(),
  };

  let options = CollabOptions::new(doc_id.to_string(), default_client_id())
    .with_data_source(data_source.into());
  let client = CollabClient::new(uid, "1");
  let mut collab = Collab::new_with_options(CollabOrigin::Client(client), options).unwrap();
  collab.add_plugin(Box::new(disk_plugin));

  collab.initialize();
  Document::open(collab).unwrap()
}

pub fn document_storage() -> Arc<CollabKVDB> {
  let tempdir = TempDir::new().unwrap();
  let path = tempdir.into_path();
  Arc::new(CollabKVDB::open(path).unwrap())
}

fn setup_log() {
  static START: Once = Once::new();
  START.call_once(|| {
    let level = "info";
    let mut filters = vec![];
    filters.push(format!("collab_persistence={}", level));
    filters.push(format!("collab={}", level));
    filters.push(format!("collab_database={}", level));
    unsafe {
      std::env::set_var("RUST_LOG", filters.join(","));
    }

    let subscriber = Subscriber::builder()
      .with_env_filter(EnvFilter::from_default_env())
      .with_ansi(true)
      .finish();
    subscriber.try_init().unwrap();
  });
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

pub fn apply_actions(document: &mut Document, actions: Vec<BlockAction>) {
  if let Err(err) = document.apply_action(actions) {
    // Handle the error
    tracing::error!("[Document] apply_action error: {:?}", err);
  }
}

pub fn insert_block_for_page(document: &mut Document, block_id: String) -> Block {
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

  document.insert_block(block, None).unwrap()
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

/// Can remove in the future. Just want to test the encode_collab and decode_collab
pub fn try_decode_from_encode_collab(document: &Document) {
  let data = document.encode_collab().unwrap();
  let options =
    CollabOptions::new("1".to_string(), default_client_id()).with_data_source(data.into());
  let _ = Collab::new_with_options(CollabOrigin::Empty, options).unwrap();
}
