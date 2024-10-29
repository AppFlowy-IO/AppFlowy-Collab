use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use crate::setup_log;

use collab::lock::RwLock;
use collab::preclude::*;
use collab_entity::CollabType;
use collab_plugins::local_storage::kv::doc::CollabKVAction;
use collab_plugins::local_storage::kv::KVTransactionDB;
use collab_plugins::local_storage::rocksdb::rocksdb_plugin::RocksdbDiskPlugin;
use collab_plugins::local_storage::rocksdb::util::KVDBCollabPersistenceImpl;
use collab_plugins::local_storage::CollabPersistenceConfig;
use collab_plugins::CollabKVDB;
use tempfile::TempDir;
use uuid::Uuid;

pub enum Script {
  CreateDocumentWithCollabDB {
    id: String,
    db: Arc<CollabKVDB>,
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
  AssertUpdateLen {
    id: String,
    expected: usize,
  },
  AssertNumOfDocuments {
    expected: usize,
  },
}

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

  pub async fn run_scripts(&mut self, scripts: Vec<Script>) {
    for script in scripts {
      self.run_script(script).await;
    }
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
    let mut collab = CollabBuilder::new(1, &doc_id, data_source.into())
      .with_device_id("1")
      .with_plugin(disk_plugin)
      .build()
      .unwrap();

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
    let mut collab = CollabBuilder::new(1, id, data_source.into())
      .with_device_id("1")
      .with_plugin(disk_plugin)
      .build()
      .unwrap();

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

  pub async fn run_script(&mut self, script: Script) {
    match script {
      Script::CreateDocumentWithCollabDB { id, db } => {
        let uid = 1;
        let disk_plugin = disk_plugin_with_db(
          self.uid,
          self.workspace_id.clone(),
          db,
          &id,
          CollabType::Unknown,
        );
        let data_source = KVDBCollabPersistenceImpl {
          db: Arc::downgrade(&self.db),
          uid,
          workspace_id: self.workspace_id.clone(),
        };
        let mut collab = CollabBuilder::new(uid, id.clone(), data_source.into())
          .with_device_id("1")
          .with_plugin(disk_plugin)
          .build()
          .unwrap();
        collab.initialize();
        self.collab_by_id.insert(id, Arc::new(RwLock::from(collab)));
      },
      Script::OpenDocument { id } => {
        self.create_collab(id).await;
      },
      Script::CloseDocument { id } => {
        self.collab_by_id.remove(&id);
      },
      Script::OpenDocumentWithDiskPlugin { id } => {
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
        let mut collab = CollabBuilder::new(1, id.clone(), data_source.into())
          .with_device_id("1")
          .with_plugin(disk_plugin)
          .build()
          .unwrap();

        collab.initialize();
        self.collab_by_id.insert(id, Arc::new(RwLock::from(collab)));
      },
      Script::DeleteDocument { id } => {
        self
          .db
          .with_write_txn(|store| store.delete_doc(self.uid, &self.workspace_id, &id))
          .unwrap();
      },
      Script::InsertKeyValue { id, key, value } => {
        self.insert(&id, key, value).await;
      },
      Script::GetValue { id, key, expected } => {
        let collab = self.collab_by_id.get(&id).unwrap().read().await;
        let txn = collab.transact();
        let text = collab
          .get_with_txn(&txn, &key)
          .map(|value| value.to_string(&txn))
          .map(|value| Any::String(Arc::from(value)));
        assert_eq!(text, expected)
      },
      Script::AssertUpdateLen { id, expected } => {
        let updates = self
          .db
          .read_txn()
          .get_decoded_v1_updates(self.uid, &self.workspace_id, &id)
          .unwrap();
        assert_eq!(updates.len(), expected)
      },
      Script::AssertNumOfDocuments { expected } => {
        let docs = self.db.read_txn().get_all_docs().unwrap();
        let docs_2 = self
          .db
          .read_txn()
          .get_all_docs_for_user(self.uid, &self.workspace_id)
          .unwrap();
        assert_eq!(docs.count(), expected);
        assert_eq!(docs_2.count(), expected);
      },
    }
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
  let collab_type = collab_type.clone();
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
