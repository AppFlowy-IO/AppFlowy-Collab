use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use collab::core::collab::MutexCollab;
use collab::preclude::*;
use collab_persistence::doc::YrsDocAction;
use collab_persistence::kv::rocks_kv::RocksCollabDB;
use collab_plugins::cloud_storage::CollabObject;
use collab_plugins::disk::rocksdb::{CollabPersistenceConfig, RocksdbDiskPlugin};
use collab_plugins::snapshot::CollabSnapshotPlugin;
use yrs::updates::decoder::Decode;

use lib0::any::Any;
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
  AssertSnapshot {
    id: String,
    index: u32,
    expected: JsonValue,
  },
  AssertNumOfUpdates {
    id: String,
    expected: usize,
  },
  AssertNumOfSnapshots {
    id: String,
    expected: usize,
  },
  AssertNumOfDocuments {
    expected: usize,
  },
  AssertDocument {
    id: String,
    expected: JsonValue,
  },
  Wait(u64),
}

pub struct CollabPersistenceTest {
  pub uid: i64,
  collab_by_id: HashMap<String, Arc<MutexCollab>>,
  #[allow(dead_code)]
  cleaner: Cleaner,
  db: Arc<RocksCollabDB>,
  disk_plugin: Arc<RocksdbDiskPlugin>,
  config: CollabPersistenceConfig,
}

impl CollabPersistenceTest {
  pub fn new(config: CollabPersistenceConfig) -> Self {
    setup_log();
    let tempdir = TempDir::new().unwrap();
    let db_path = tempdir.into_path();
    let uid = 1;
    let db = Arc::new(RocksCollabDB::open(db_path.clone()).unwrap());
    let disk_plugin = Arc::new(RocksdbDiskPlugin::new_with_config(
      uid,
      db.clone(),
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

  fn make_snapshot_plugin(
    &self,
    object_id: String,
    _collab: Arc<MutexCollab>,
  ) -> Arc<CollabSnapshotPlugin> {
    let object = CollabObject::new(object_id);
    Arc::new(CollabSnapshotPlugin::new(
      self.uid,
      object,
      Arc::new(self.db.clone()),
      self.db.clone(),
      self.config.snapshot_per_update,
    ))
  }

  pub async fn create_collab(&mut self, doc_id: String) {
    let collab = Arc::new(CollabBuilder::new(1, &doc_id).build());
    collab.lock().add_plugin(self.disk_plugin.clone());
    let object_id = collab.lock().object_id.clone();
    collab
      .lock()
      .add_plugin(self.make_snapshot_plugin(object_id, collab.clone()));
    collab.initial();

    self.collab_by_id.insert(doc_id, collab);
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
    let collab = Arc::new(CollabBuilder::new(1, id).build());
    collab.lock().add_plugin(self.disk_plugin.clone());
    collab
      .lock()
      .add_plugin(self.make_snapshot_plugin(id.to_string(), collab.clone()));
    collab.initial();

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
            .with_plugin(plugin.clone())
            .build(),
        );
        self.disk_plugin = Arc::new(plugin);
        let object_id = collab.lock().object_id.clone();
        collab
          .lock()
          .add_plugin(self.make_snapshot_plugin(object_id, collab.clone()));
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
          .with_plugin(self.disk_plugin.clone())
          .build();
        collab.initial();
        self.collab_by_id.insert(id, Arc::new(collab));
      },
      Script::DeleteDocument { id } => {
        self
          .disk_plugin
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
          .map(|value| Any::String(value.into_boxed_str()));
        assert_eq!(text, expected)
      },
      Script::AssertNumOfUpdates { id, expected } => {
        let updates = self
          .disk_plugin
          .read_txn()
          .get_decoded_v1_updates(self.uid, &id)
          .unwrap();
        assert_eq!(updates.len(), expected)
      },
      Script::AssertNumOfSnapshots { id, expected } => {
        let snapshot_plugin =
          self.make_snapshot_plugin(id.clone(), self.collab_by_id.get(&id).unwrap().clone());
        let snapshot = snapshot_plugin.get_snapshots(&id);
        assert_eq!(snapshot.len(), expected);
      },
      Script::AssertNumOfDocuments { expected } => {
        let docs = self.disk_plugin.read_txn().get_all_docs().unwrap();
        assert_eq!(docs.count(), expected);
      },
      Script::AssertSnapshot {
        id,
        index,
        expected,
      } => {
        let snapshot_plugin =
          self.make_snapshot_plugin(id.clone(), self.collab_by_id.get(&id).unwrap().clone());
        let snapshots = snapshot_plugin.get_snapshots(&id);
        let collab = CollabBuilder::new(1, &id).build();
        collab.lock().with_transact_mut(|txn| {
          txn.apply_update(Update::decode_v1(&snapshots[index as usize].data).unwrap());
        });

        let json = collab.lock().to_json_value();
        assert_json_diff::assert_json_eq!(json, expected);
      },
      Script::AssertDocument { id, expected } => {
        let collab = Arc::new(CollabBuilder::new(1, &id).build());
        collab.lock().add_plugin(self.disk_plugin.clone());
        collab
          .lock()
          .add_plugin(self.make_snapshot_plugin(id, collab.clone()));
        collab.initial();

        let json = collab.to_json_value();
        assert_json_diff::assert_json_eq!(json, expected);
      },
      Script::Wait(secs) => {
        tokio::time::sleep(Duration::from_secs(secs)).await;
      },
    }
  }
}

pub fn disk_plugin(uid: i64) -> RocksdbDiskPlugin {
  let tempdir = TempDir::new().unwrap();
  let path = tempdir.into_path();
  let db = Arc::new(RocksCollabDB::open(path).unwrap());
  RocksdbDiskPlugin::new(uid, db)
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
