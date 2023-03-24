use collab::plugin_impl::disk::CollabDiskPlugin;
use collab::preclude::CollabBuilder;
use collab_folder::core::{Folder, Workspace};
use collab_persistence::CollabKV;
use std::ops::Deref;
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::TempDir;

pub struct FolderTest {
    folder: Folder,
    cleaner: Cleaner,
}

pub fn create_folder(id: &str) -> FolderTest {
    let tempdir = TempDir::new().unwrap();
    let path = tempdir.into_path();
    let db = Arc::new(CollabKV::open(path.clone()).unwrap());
    let disk_plugin = CollabDiskPlugin::new(db).unwrap();
    let cleaner = Cleaner::new(path);

    let collab = CollabBuilder::new(1, id).with_plugin(disk_plugin).build();
    let folder = Folder::create(collab);
    FolderTest { folder, cleaner }
}

pub fn create_folder_with_workspace(id: &str, workspace_id: &str) -> FolderTest {
    let test = create_folder(id);
    let workspace = Workspace {
        id: workspace_id.to_string(),
        name: "My first workspace".to_string(),
        belongings: Default::default(),
        created_at: 123,
    };

    test.folder.workspaces.create_workspace(workspace);
    test
}

impl Deref for FolderTest {
    type Target = Folder;

    fn deref(&self) -> &Self::Target {
        &self.folder
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
