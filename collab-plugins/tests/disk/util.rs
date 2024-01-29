use std::path::PathBuf;

use collab_plugins::CollabKVDB;
use tempfile::TempDir;

pub fn rocks_db() -> (PathBuf, CollabKVDB) {
  let tempdir = TempDir::new().unwrap();
  let path = tempdir.into_path();
  let cloned_path = path.clone();
  (path, CollabKVDB::open(cloned_path).unwrap())
}
