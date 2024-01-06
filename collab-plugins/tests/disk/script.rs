use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use collab::core::collab::MutexCollab;
use collab::preclude::*;

use collab_plugins::local_storage::CollabPersistenceConfig;

use collab_plugins::local_storage::kv::doc::CollabKVAction;
use collab_plugins::local_storage::rocksdb::rocksdb_plugin::RocksdbDiskPlugin;

use collab_plugins::CollabKVDB;
use tempfile::TempDir;

use crate::setup_log;

pub enum Script {
  CreateDocumentWithDiskPlugin {
    id: String,
    plugin: RocksdbDiskPlugin,
  },
  OpenDocumentWithDiskPlugin {
    id: String,
  },
  #[allow(dead_code)]
  OpenDocument {
    id: String,
  },
  CloseDocument {
    id: String,
  },
  DeleteDocument {
    id: String,
  },
  InsertKeyValue {
    id: String,
    key: String,
    value: Any,
  },
  GetValue {
    id: String,
    key: String,
    expected: Option<Any>,
  },
  AssertNumOfUpdates {
    id: String,
    expected: usize,
  },
  AssertNumOfDocuments {
    expected: usize,
  },
}

pub struct CollabPersistenceTest {
  pub uid: i64,
  collab_by_id: HashMap<String, Arc<MutexCollab>>,
  #[allow(dead_code)]
  cleaner: Cleaner,
  #[allow(dead_code)]
  db: Arc<CollabKVDB>,
  disk_plugin: Arc<RocksdbDiskPlugin>,
  #[allow(dead_code)]
  config: CollabPersistenceConfig,
}

impl CollabPersistenceTest {
  pub fn new(config: CollabPersistenceConfig) -> Self {
    setup_log();
    let tempdir = TempDir::new().unwrap();
    let db_path = tempdir.into_path();
    let uid = 1;
    let db = Arc::new(CollabKVDB::open_opt(db_path.clone(), false).unwrap());
    let disk_plugin = Arc::new(RocksdbDiskPlugin::new_with_config(
      uid,
      Arc::downgrade(&db),
      config.clone(),
    ));
    let cleaner = Cleaner::new(db_path);
    Self {
      uid,
      collab_by_id: HashMap::default(),
      disk_plugin,
      cleaner,
      db,
      config,
    }
  }

  pub async fn run_scripts(&mut self, scripts: Vec<Script>) {
    for script in scripts {
      self.run_script(script).await;
    }
  }

  pub async fn create_collab(&mut self, doc_id: String) {
    let collab = Arc::new(
      CollabBuilder::new(1, &doc_id)
        .with_device_id("1")
        .build()
        .unwrap(),
    );
    collab.lock().add_plugin(self.disk_plugin.clone());
    collab.lock().initialize();

    self.collab_by_id.insert(doc_id, collab);
  }

  pub async fn enable_undo_redo(&self, doc_id: &str) {
    self
      .collab_by_id
      .get(doc_id)
      .as_ref()
      .unwrap()
      .lock()
      .enable_undo_redo();
  }

  pub async fn insert(&mut self, id: &str, key: String, value: Any) {
    self
      .collab_by_id
      .get(id)
      .as_ref()
      .unwrap()
      .lock()
      .insert(&key, value);
  }

  pub async fn assert_collab(&mut self, id: &str, expected: JsonValue) {
    let collab = Arc::new(
      CollabBuilder::new(1, id)
        .with_device_id("1")
        .build()
        .unwrap(),
    );
    collab.lock().add_plugin(self.disk_plugin.clone());
    collab.lock().initialize();

    let json = collab.to_json_value();
    assert_json_diff::assert_json_eq!(json, expected);
  }

  pub async fn undo(&mut self, id: &str) {
    self
      .collab_by_id
      .get(id)
      .as_ref()
      .unwrap()
      .lock()
      .undo()
      .unwrap();
  }

  pub async fn redo(&mut self, id: &str) {
    self
      .collab_by_id
      .get(id)
      .as_ref()
      .unwrap()
      .lock()
      .redo()
      .unwrap();
  }

  pub async fn run_script(&mut self, script: Script) {
    match script {
      Script::CreateDocumentWithDiskPlugin { id, plugin } => {
        let collab = Arc::new(
          CollabBuilder::new(1, &id)
            .with_device_id("1")
            .with_plugin(plugin.clone())
            .build()
            .unwrap(),
        );
        self.disk_plugin = Arc::new(plugin);
        collab.lock().initialize();
        self.collab_by_id.insert(id, collab);
      },
      Script::OpenDocument { id } => {
        self.create_collab(id).await;
      },
      Script::CloseDocument { id } => {
        self.collab_by_id.remove(&id);
      },
      Script::OpenDocumentWithDiskPlugin { id } => {
        let collab = CollabBuilder::new(1, &id)
          .with_device_id("1")
          .with_plugin(self.disk_plugin.clone())
          .build()
          .unwrap();
        collab.lock().initialize();
        self.collab_by_id.insert(id, Arc::new(collab));
      },
      Script::DeleteDocument { id } => {
        let collab_db = self.disk_plugin.upgrade().unwrap();
        collab_db
          .with_write_txn(|store| store.delete_doc(self.uid, &id))
          .unwrap();
      },
      Script::InsertKeyValue { id, key, value } => {
        self.insert(&id, key, value).await;
      },
      Script::GetValue { id, key, expected } => {
        let collab = self.collab_by_id.get(&id).unwrap().lock();
        let txn = collab.transact();
        let text = collab
          .get(&key)
          .map(|value| value.to_string(&txn))
          .map(|value| Any::String(Arc::from(value)));
        assert_eq!(text, expected)
      },
      Script::AssertNumOfUpdates { id, expected } => {
        let collab_db = self.disk_plugin.upgrade().unwrap();
        let updates = collab_db
          .read_txn()
          .get_decoded_v1_updates(self.uid, &id)
          .unwrap();
        assert_eq!(updates.len(), expected)
      },
      Script::AssertNumOfDocuments { expected } => {
        let collab_db = self.disk_plugin.upgrade().unwrap();

        let docs = collab_db.read_txn().get_all_docs().unwrap();
        assert_eq!(docs.count(), expected);
      },
    }
  }
}

pub fn disk_plugin(uid: i64) -> (Arc<CollabKVDB>, RocksdbDiskPlugin) {
  let tempdir = TempDir::new().unwrap();
  let path = tempdir.into_path();
  let db = Arc::new(CollabKVDB::open_opt(path, false).unwrap());
  let plugin = RocksdbDiskPlugin::new_with_config(
    uid,
    Arc::downgrade(&db),
    CollabPersistenceConfig::default(),
  );
  (db, plugin)
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
