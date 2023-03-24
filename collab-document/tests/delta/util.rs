use collab::plugin_impl::disk::CollabDiskPlugin;
use collab::preclude::CollabBuilder;

use collab_document::document::Document;
use std::path::PathBuf;
use tempfile::TempDir;

pub struct DocumentTest {
    pub document: Document,
    #[allow(dead_code)]
    cleaner: Cleaner,
}

pub fn create_document(doc_id: &str) -> DocumentTest {
    let tempdir = TempDir::new().unwrap();
    let path = tempdir.into_path();
    let disk_plugin = CollabDiskPlugin::new(path.clone()).unwrap();
    let cleaner = Cleaner::new(path);

    let collab = CollabBuilder::new(1, doc_id)
        .with_plugin(disk_plugin)
        .build();

    let document = Document::create(collab);
    DocumentTest { document, cleaner }
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
