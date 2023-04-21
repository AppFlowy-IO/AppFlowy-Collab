use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use collab::plugin_impl::rocks_disk::RocksDiskPlugin;
use collab::plugin_impl::rocks_snapshot::RocksSnapshotPlugin;
use collab::preclude::*;
use collab_persistence::doc::YrsDocAction;
use collab_persistence::kv::rocks_kv::RocksCollabDB;
use collab_persistence::snapshot::SnapshotAction;
use lib0::any::Any;
use yrs::updates::decoder::Decode;

use tempfile::TempDir;

pub enum Script {
  CreateDocumentWithDiskPlugin {
    id: String,
    plugin: RocksDiskPlugin,
  },
  OpenDocumentWithDiskPlugin {
    id: String,
  },
  OpenDocumentWithSnapshotPlugin {
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
  AssertNumOfDocuments {
    expected: usize,
  },
}

pub struct CollabPersistenceTest {
  pub uid: i64,
  collabs: HashMap<String, Collab>,
  disk_plugin: RocksDiskPlugin,
  snapshot_plugin: RocksSnapshotPlugin,
  #[allow(dead_code)]
  cleaner: Cleaner,
  pub db_path: PathBuf,
}

impl CollabPersistenceTest {
  pub fn new() -> Self {
    let tempdir = TempDir::new().unwrap();
    let path = tempdir.into_path();
    let uid = 1;
    let db = Arc::new(RocksCollabDB::open(path.clone()).unwrap());
    let disk_plugin = RocksDiskPlugin::new(uid, db.clone()).unwrap();
    let snapshot_plugin = RocksSnapshotPlugin::new(uid, db, 5).unwrap();
    let cleaner = Cleaner::new(path.clone());
    Self {
      uid,
      collabs: HashMap::default(),
      disk_plugin,
      snapshot_plugin,
      cleaner,
      db_path: path,
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
        let mut collab = CollabBuilder::new(1, &id).build();
        collab.add_plugins(vec![Arc::new(plugin.clone())]);
        collab.initial();

        self.disk_plugin = plugin;
        self.collabs.insert(id, collab);
      },
      Script::OpenDocumentWithSnapshotPlugin { id } => {
        let collab = CollabBuilder::new(1, &id)
          .with_plugin(self.snapshot_plugin.clone())
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
        self.collabs.get(&id).as_ref().unwrap().insert(&key, value);
      },
      Script::GetValue { id, key, expected } => {
        let collab = self.collabs.get(&id).unwrap();
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
          .get_updates(self.uid, &id)
          .unwrap();
        assert_eq!(updates.len(), expected)
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
        let snapshot = self.snapshot_plugin.db.read_txn();
        let snapshots = snapshot.get_snapshots(self.snapshot_plugin.uid, &id);
        let collab = CollabBuilder::new(1, &id).build();
        collab.with_transact_mut(|txn| {
          txn.apply_update(Update::decode_v1(&snapshots[index as usize].data).unwrap());
        });

        let json = collab.to_json_value();
        assert_json_diff::assert_json_eq!(json, expected);
      },
    }
  }
}

pub fn disk_plugin(uid: i64) -> RocksDiskPlugin {
  let tempdir = TempDir::new().unwrap();
  let path = tempdir.into_path();
  let db = Arc::new(RocksCollabDB::open(path).unwrap());
  RocksDiskPlugin::new(uid, db).unwrap()
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
