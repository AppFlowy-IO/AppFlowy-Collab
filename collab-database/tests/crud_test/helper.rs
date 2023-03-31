use collab::plugin_impl::disk::CollabDiskPlugin;
use collab::preclude::CollabBuilder;
use collab_database::database::{Database, DatabaseContext};
use collab_persistence::CollabKV;
use std::ops::{Deref, DerefMut};
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::TempDir;

pub struct DatabaseTest {
  database: Database,

  #[allow(dead_code)]
  cleaner: Cleaner,
}

unsafe impl Send for DatabaseTest {}
unsafe impl Sync for DatabaseTest {}

impl Deref for DatabaseTest {
  type Target = Database;

  fn deref(&self) -> &Self::Target {
    &self.database
  }
}

impl DerefMut for DatabaseTest {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.database
  }
}

pub fn create_database(id: &str) -> DatabaseTest {
  let uid = 1;
  let tempdir = TempDir::new().unwrap();
  let path = tempdir.into_path();
  let db = Arc::new(CollabKV::open(path.clone()).unwrap());
  let disk_plugin = CollabDiskPlugin::new(uid, db).unwrap();
  let cleaner = Cleaner::new(path);

  let collab = CollabBuilder::new(1, id).with_plugin(disk_plugin).build();
  collab.initial();
  let context = DatabaseContext {};
  let database = Database::create(collab, context);
  DatabaseTest { database, cleaner }
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
