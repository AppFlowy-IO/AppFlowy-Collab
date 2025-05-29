use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use crate::setup_log;
use collab::core::collab::{CollabOptions, default_client_id};
use collab::core::origin::CollabOrigin;
use collab::lock::RwLock;
use collab::preclude::*;
use collab_entity::CollabType;
use collab_plugins::CollabKVDB;
use collab_plugins::local_storage::CollabPersistenceConfig;
use collab_plugins::local_storage::kv::KVTransactionDB;
use collab_plugins::local_storage::kv::doc::CollabKVAction;
use collab_plugins::local_storage::rocksdb::rocksdb_plugin::RocksdbDiskPlugin;
use collab_plugins::local_storage::rocksdb::util::KVDBCollabPersistenceImpl;
use tempfile::TempDir;
use uuid::Uuid;

pub struct CollabPersistenceTest {
  pub uid: i64,
  pub workspace_id: String,
  collab_by_id: HashMap<String, Arc<RwLock<Collab>>>,
  #[allow(dead_code)]
  cleaner: Cleaner,
  #[allow(dead_code)]
  pub db: Arc<CollabKVDB>,
  #[allow(dead_code)]
  config: CollabPersistenceConfig,
}

impl CollabPersistenceTest {
  pub fn new(config: CollabPersistenceConfig) -> Self {
    setup_log();
    let workspace_id = Uuid::new_v4().to_string();
    let tempdir = TempDir::new().unwrap();
    let db_path = tempdir.into_path();
    let uid = 1;
    let db = Arc::new(CollabKVDB::open(db_path.clone()).unwrap());
    let cleaner = Cleaner::new(db_path);
    Self {
      uid,
      workspace_id,
      collab_by_id: HashMap::default(),
      cleaner,
      db,
      config,
    }
  }

  pub async fn create_document_with_collab_db(&mut self, id: String, db: Arc<CollabKVDB>) {
    let disk_plugin = disk_plugin_with_db(
      self.uid,
      self.workspace_id.clone(),
      db,
      &id,
      CollabType::Unknown,
    );
    let data_source = KVDBCollabPersistenceImpl {
      db: Arc::downgrade(&self.db),
      uid: self.uid,
      workspace_id: self.workspace_id.clone(),
    };

    let options =
      CollabOptions::new(id.clone(), default_client_id()).with_data_source(data_source.into());
    let mut collab = Collab::new_with_options(CollabOrigin::Empty, options).unwrap();
    collab.add_plugin(Box::new(disk_plugin));
    collab.initialize();
    self.collab_by_id.insert(id, Arc::new(RwLock::from(collab)));
  }

  pub async fn open_document_with_disk_plugin(&mut self, id: String) {
    let disk_plugin = disk_plugin_with_db(
      self.uid,
      self.workspace_id.clone(),
      self.db.clone(),
      &id,
      CollabType::Unknown,
    );
    let data_source = KVDBCollabPersistenceImpl {
      db: Arc::downgrade(&self.db),
      uid: self.uid,
      workspace_id: self.workspace_id.clone(),
    };

    let options =
      CollabOptions::new(id.clone(), default_client_id()).with_data_source(data_source.into());
    let mut collab = Collab::new_with_options(CollabOrigin::Empty, options).unwrap();
    collab.add_plugin(Box::new(disk_plugin));
    collab.initialize();
    self.collab_by_id.insert(id, Arc::new(RwLock::from(collab)));
  }

  pub async fn close_document(&mut self, id: String) {
    self.collab_by_id.remove(&id);
  }

  pub async fn delete_document(&mut self, id: String) {
    self
      .db
      .with_write_txn(|store| store.delete_doc(self.uid, &self.workspace_id, &id))
      .unwrap();
  }

  pub async fn insert_key_value(&mut self, id: String, key: String, value: Any) {
    self.insert(&id, key, value).await;
  }

  pub async fn get_value(&mut self, id: String, key: String, expected: Option<Any>) {
    let collab = self.collab_by_id.get(&id).unwrap().read().await;
    let txn = collab.transact();
    let text = collab
      .get_with_txn(&txn, &key)
      .map(|value| value.to_string(&txn))
      .map(|value| Any::String(Arc::from(value)));
    assert_eq!(text, expected);
  }

  pub async fn assert_update_len(&mut self, id: String, expected: usize) {
    let updates = self
      .db
      .read_txn()
      .get_decoded_v1_updates(self.uid, &self.workspace_id, &id)
      .unwrap();
    assert_eq!(updates.len(), expected);
  }

  pub async fn assert_ids(&mut self, mut expected: Vec<String>) {
    let mut docs = self
      .db
      .read_txn()
      .get_all_object_ids(self.uid, &self.workspace_id)
      .map(|iter| iter.collect::<Vec<String>>())
      .unwrap_or_default();
    docs.sort();
    expected.sort();
    assert_eq!(docs, expected);
  }

  pub async fn create_collab(&mut self, doc_id: String) {
    let disk_plugin = disk_plugin_with_db(
      self.uid,
      self.workspace_id.clone(),
      self.db.clone(),
      &doc_id,
      CollabType::Unknown,
    );
    let data_source = KVDBCollabPersistenceImpl {
      db: Arc::downgrade(&self.db),
      uid: self.uid,
      workspace_id: self.workspace_id.clone(),
    };

    let options =
      CollabOptions::new(doc_id.clone(), default_client_id()).with_data_source(data_source.into());
    let mut collab = Collab::new_with_options(CollabOrigin::Empty, options).unwrap();
    collab.add_plugin(Box::new(disk_plugin));
    collab.initialize();
    self
      .collab_by_id
      .insert(doc_id, Arc::new(RwLock::from(collab)));
  }

  pub async fn enable_undo_redo(&self, doc_id: &str) {
    self
      .collab_by_id
      .get(doc_id)
      .as_ref()
      .unwrap()
      .write()
      .await
      .enable_undo_redo();
  }

  pub async fn insert(&mut self, id: &str, key: String, value: Any) {
    self
      .collab_by_id
      .get(id)
      .as_ref()
      .unwrap()
      .write()
      .await
      .insert(&key, value);
  }

  pub async fn assert_collab(&mut self, id: &str, expected: JsonValue) {
    let disk_plugin = disk_plugin_with_db(
      self.uid,
      self.workspace_id.clone(),
      self.db.clone(),
      id,
      CollabType::Document,
    );
    let data_source = KVDBCollabPersistenceImpl {
      db: Arc::downgrade(&self.db),
      uid: self.uid,
      workspace_id: self.workspace_id.clone(),
    };

    let options =
      CollabOptions::new(id.to_string(), default_client_id()).with_data_source(data_source.into());
    let mut collab = Collab::new_with_options(CollabOrigin::Empty, options).unwrap();
    collab.add_plugin(Box::new(disk_plugin));
    collab.initialize();
    let json = collab.to_json_value();
    assert_json_diff::assert_json_eq!(json, expected);
  }

  pub async fn undo(&mut self, id: &str) {
    self
      .collab_by_id
      .get(id)
      .as_ref()
      .unwrap()
      .write()
      .await
      .undo()
      .unwrap();
  }

  pub async fn redo(&mut self, id: &str) {
    self
      .collab_by_id
      .get(id)
      .as_ref()
      .unwrap()
      .write()
      .await
      .redo()
      .unwrap();
  }
}

pub fn disk_plugin_with_db(
  uid: i64,
  workspace_id: String,
  db: Arc<CollabKVDB>,
  object_id: &str,
  collab_type: CollabType,
) -> Box<RocksdbDiskPlugin> {
  let object_id = object_id.to_string();
  Box::new(RocksdbDiskPlugin::new_with_config(
    uid,
    workspace_id,
    object_id,
    collab_type,
    Arc::downgrade(&db),
    CollabPersistenceConfig::default(),
  ))
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
