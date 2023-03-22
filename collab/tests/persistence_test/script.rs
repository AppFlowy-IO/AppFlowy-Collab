use collab::core::collab::{Collab, CollabBuilder};
use collab::plugin_impl::disk::CollabDiskPlugin;
use collab_derive::Collab;
use lib0::any::Any;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;
use tempfile::TempDir;
use yrs::Map;

pub enum Script {
    InsertText { key: String, value: Any },
    GetText { key: String, expected: Option<Any> },
    AssertNumOfUpdates { expected: usize },
    AssertDiskUpdate,
}

pub struct CollabPersistenceTest {
    collab: Collab,
    disk_plugin: CollabDiskPlugin,
    pub db_path: PathBuf,
    pub cid: String,
}

impl CollabPersistenceTest {
    pub fn new() -> Self {
        let tempdir = TempDir::new().unwrap();
        let path = tempdir.into_path();
        // let cleaner = Cleaner::new(path.clone());
        let cid = "1".to_string();
        let disk_plugin = CollabDiskPlugin::new(path.clone()).unwrap();
        let collab = CollabBuilder::new(1, &cid)
            .with_plugin(disk_plugin.clone())
            .build();
        Self {
            collab,
            disk_plugin,
            db_path: path,
            cid,
        }
    }

    pub fn new_with_path(path: PathBuf, cid: String) -> Self {
        let disk_plugin = CollabDiskPlugin::new(path.clone()).unwrap();
        let collab = CollabBuilder::new(1, &cid)
            .with_plugin(disk_plugin.clone())
            .build();
        Self {
            collab,
            disk_plugin,
            db_path: path,
            cid,
        }
    }

    pub fn run_scripts(&self, scripts: Vec<Script>) {
        for script in scripts {
            self.run_script(script);
        }
    }

    pub fn run_script(&self, script: Script) {
        match script {
            Script::InsertText { key, value } => {
                self.collab.insert(&key, value);
            }
            Script::GetText { key, expected } => {
                let txn = self.collab.transact();
                let text = self
                    .collab
                    .get(&key)
                    .map(|value| value.to_string(&txn))
                    .map(|value| Any::String(value.into_boxed_str()));
                assert_eq!(text, expected)
            }
            Script::AssertNumOfUpdates { expected } => {
                let updates = self.disk_plugin.doc().get_updates(&self.cid).unwrap();
                assert_eq!(updates.len(), expected)
            }
            Script::AssertDiskUpdate => {
                //
            }
        }
    }
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

#[derive(Collab, Serialize, Deserialize, Clone)]
pub struct Document {
    doc_id: String,
    name: String,
    created_at: i64,
    attributes: HashMap<String, String>,
}
