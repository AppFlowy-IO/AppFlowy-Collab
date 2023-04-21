use std::ops::Deref;
use std::path::PathBuf;
use std::sync::{Arc, Once};

use collab::preclude::CollabBuilder;
use collab_folder::core::{
  Belonging, Belongings, Folder, FolderContext, TrashChangeReceiver, View, ViewChangeReceiver,
  ViewLayout, Workspace,
};

use collab::plugin_impl::rocks_disk::RocksDiskPlugin;
use collab_persistence::kv::rocks_kv::RocksCollabDB;
use tempfile::TempDir;
use tracing_subscriber::fmt::Subscriber;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;

pub struct FolderTest {
  folder: Folder,

  #[allow(dead_code)]
  cleaner: Cleaner,

  #[allow(dead_code)]
  view_rx: ViewChangeReceiver,

  #[allow(dead_code)]
  pub trash_rx: Option<TrashChangeReceiver>,
}

unsafe impl Send for FolderTest {}

unsafe impl Sync for FolderTest {}

pub fn create_folder(id: &str) -> FolderTest {
  let uid = 1;
  let tempdir = TempDir::new().unwrap();
  let path = tempdir.into_path();
  let db = Arc::new(RocksCollabDB::open(path.clone()).unwrap());
  let disk_plugin = RocksDiskPlugin::new(uid, db).unwrap();
  let cleaner: Cleaner = Cleaner::new(path);

  let collab = CollabBuilder::new(1, id).with_plugin(disk_plugin).build();
  collab.initial();

  let (view_tx, view_rx) = tokio::sync::broadcast::channel(100);
  let (trash_tx, trash_rx) = tokio::sync::broadcast::channel(100);
  let context = FolderContext {
    view_change_tx: Some(view_tx),
    trash_change_tx: Some(trash_tx),
  };
  let folder = Folder::get_or_create(collab, context);
  FolderTest {
    folder,
    cleaner,
    view_rx,
    trash_rx: Some(trash_rx),
  }
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

pub fn make_test_view(view_id: &str, bid: &str, belongings: Vec<String>) -> View {
  let belongings = belongings
    .into_iter()
    .map(Belonging::new)
    .collect::<Vec<Belonging>>();
  View {
    id: view_id.to_string(),
    bid: bid.to_string(),
    name: "".to_string(),
    desc: "".to_string(),
    belongings: Belongings::new(belongings),
    created_at: 0,
    layout: ViewLayout::Document,
    database_id: None,
  }
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

pub fn setup_log() {
  static START: Once = Once::new();
  START.call_once(|| {
    std::env::set_var("RUST_LOG", "collab_persistence=trace");
    let subscriber = Subscriber::builder()
      .with_env_filter(EnvFilter::from_default_env())
      .with_ansi(true)
      .finish();
    subscriber.try_init().unwrap();
  });
}
