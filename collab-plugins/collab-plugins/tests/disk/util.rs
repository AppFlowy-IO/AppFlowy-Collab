use std::path::PathBuf;

use collab_plugins_core::CollabKVDB;
use tempfile::TempDir;

pub fn rocks_db() -> (PathBuf, CollabKVDB) {
  let tempdir = TempDir::new().unwrap();
  let path = tempdir.into_path();
  let cloned_path = path.clone();
  (path, CollabKVDB::open_opt(cloned_path, false).unwrap())
}
