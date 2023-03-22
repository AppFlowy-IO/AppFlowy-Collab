use collab::collab::{Collab, CollabBuilder};
use collab::plugin_impl::disk::CollabDiskPlugin;
use std::path::PathBuf;
use tempfile::TempDir;

pub struct CollabPersistenceTest {
    pub collab: Collab,
    cleaner: Cleaner,
}

impl CollabPersistenceTest {
    pub fn new() -> Self {
        let tempdir = TempDir::new().unwrap();
        let path = tempdir.into_path();
        let cleaner = Cleaner::new(path.clone());
        let disk_plugin = CollabDiskPlugin::new(path).unwrap();

        let collab = CollabBuilder::new(1).with_plugin(disk_plugin).build();
        Self { collab, cleaner }
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
