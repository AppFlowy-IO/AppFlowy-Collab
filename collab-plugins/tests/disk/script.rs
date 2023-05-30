use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use collab::core::collab::MutexCollab;
use collab::preclude::*;
use collab_persistence::doc::YrsDocAction;
use collab_persistence::kv::rocks_kv::RocksCollabDB;
use collab_persistence::snapshot::SnapshotAction;
use collab_plugins::disk::rocksdb::{CollabPersistenceConfig, RocksdbDiskPlugin};
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
  ValidateSnapshotUpdateKey {
    id: String,
    snapshot_index: usize,
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
}

pub struct CollabPersistenceTest {
  pub uid: i64,
  collabs: HashMap<String, MutexCollab>,
  pub disk_plugin: RocksdbDiskPlugin,
  #[allow(dead_code)]
  cleaner: Cleaner,
  db: Arc<RocksCollabDB>,
  config: CollabPersistenceConfig,
}

impl CollabPersistenceTest {
  pub fn new(config: CollabPersistenceConfig) -> Self {
    setup_log();
    let tempdir = TempDir::new().unwrap();
    let db_path = tempdir.into_path();
    let uid = 1;
    let db = Arc::new(RocksCollabDB::open(db_path.clone()).unwrap());
    let disk_plugin = RocksdbDiskPlugin::new_with_config(uid, db.clone(), config.clone());
    let cleaner = Cleaner::new(db_path);
    Self {
      uid,
      collabs: HashMap::default(),
      disk_plugin,
      cleaner,
      db,
      config,
    }
  }

  pub fn run_scripts(&mut self, scripts: Vec<Script>) {
    for script in scripts {
      self.run_script(script);
    }
  }

  pub fn run_script(&mut self, script: Script) {
    match script {
      Script::CreateDocumentWithDiskPlugin { id, plugin } => {
        let collab = CollabBuilder::new(1, &id)
          .with_plugin(plugin.clone())
          .build();
        collab.lock().initialize();

        self.disk_plugin = plugin;
        self.collabs.insert(id, collab);
      },
      Script::OpenDocument { id } => {
        self.disk_plugin =
          RocksdbDiskPlugin::new_with_config(self.uid, self.db.clone(), self.config.clone());

        let collab = CollabBuilder::new(1, &id)
          .with_plugin(self.disk_plugin.clone())
          .build();
        collab.initial();
        self.collabs.insert(id, collab);
      },
      Script::CloseDocument { id } => {
        self.collabs.remove(&id);
      },
      Script::OpenDocumentWithDiskPlugin { id } => {
        let collab = CollabBuilder::new(1, &id)
          .with_plugin(self.disk_plugin.clone())
          .build();
        collab.initial();
        self.collabs.insert(id, collab);
      },
      Script::DeleteDocument { id } => {
        self
          .disk_plugin
          .with_write_txn(|store| store.delete_doc(self.uid, &id))
          .unwrap();
      },
      Script::InsertKeyValue { id, key, value } => {
        self
          .collabs
          .get(&id)
          .as_ref()
          .unwrap()
          .lock()
          .insert(&key, value);
      },
      Script::GetValue { id, key, expected } => {
        let collab = self.collabs.get(&id).unwrap().lock();
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
        let snapshot = self.disk_plugin.read_txn().get_snapshots(self.uid, &id);
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
        let snapshots = self.disk_plugin.get_snapshots(&id);
        let collab = CollabBuilder::new(1, &id).build();
        collab.lock().with_transact_mut(|txn| {
          txn.apply_update(Update::decode_v1(&snapshots[index as usize].data).unwrap());
        });

        let json = collab.lock().to_json_value();
        assert_json_diff::assert_json_eq!(json, expected);
      },
      Script::ValidateSnapshotUpdateKey { id, snapshot_index } => {
        let snapshots = self.disk_plugin.get_snapshots(&id);
        let snapshot = snapshots.get(snapshot_index).unwrap();
        let key = self
          .disk_plugin
          .read_txn()
          .get_doc_last_update_key(self.uid, &id)
          .unwrap()
          .to_vec();

        assert_eq!(key, snapshot.update_key)
      },
      Script::AssertDocument { id, expected } => {
        let mut doc = Collab::new(self.uid, id, vec![]);
        doc.add_plugin(Arc::new(self.disk_plugin.clone()));
        doc.initialize();
        let json = doc.to_json_value();
        assert_json_diff::assert_json_eq!(json, expected);
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
