use std::ops::{Deref, DerefMut};
use std::path::PathBuf;
use std::sync::{Arc, Once};

use collab::core::collab::{CollabOptions, DataSource, default_client_id};
use collab::core::origin::{CollabClient, CollabOrigin};
use collab::preclude::Collab;
use collab_entity::CollabType;
use collab_folder::*;
use collab_plugins::CollabKVDB;
use collab_plugins::local_storage::rocksdb::rocksdb_plugin::RocksdbDiskPlugin;
use tempfile::TempDir;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt::Subscriber;
use tracing_subscriber::util::SubscriberInitExt;

pub struct FolderTest {
  pub folder: Folder,

  #[allow(dead_code)]
  db: Arc<CollabKVDB>,

  #[allow(dead_code)]
  cleaner: Cleaner,

  #[allow(dead_code)]
  view_rx: ViewChangeReceiver,

  #[allow(dead_code)]
  pub(crate) section_rx: Option<SectionChangeReceiver>,
}

pub fn create_folder(uid: UserId, workspace_id: &str) -> FolderTest {
  let mut workspace = Workspace::new(workspace_id.to_string(), "".to_string(), uid.as_i64());
  workspace.created_at = 0;
  let folder_data = FolderData::new(uid.as_i64(), workspace);
  create_folder_with_data(uid, workspace_id, folder_data)
}

pub fn create_folder_with_data(
  uid: UserId,
  workspace_id: &str,
  folder_data: FolderData,
) -> FolderTest {
  let tempdir = TempDir::new().unwrap();

  let path = tempdir.into_path();
  let db = Arc::new(CollabKVDB::open(path.clone()).unwrap());
  let disk_plugin = RocksdbDiskPlugin::new(
    uid.as_i64(),
    workspace_id.to_string(),
    workspace_id.to_string(),
    CollabType::Folder,
    Arc::downgrade(&db),
  );
  let cleaner: Cleaner = Cleaner::new(path);

  let options = CollabOptions::new(workspace_id.to_string(), default_client_id())
    .with_data_source(DataSource::Disk(None));
  let client = CollabClient::new(uid.as_i64(), "1");
  let mut collab = Collab::new_with_options(CollabOrigin::Client(client), options).unwrap();
  collab.add_plugin(Box::new(disk_plugin));
  collab.initialize();

  let (view_tx, view_rx) = tokio::sync::broadcast::channel(100);
  let (section_tx, section_rx) = tokio::sync::broadcast::channel(100);
  let context = FolderNotify {
    view_change_tx: view_tx,
    section_change_tx: section_tx,
  };
  let folder = Folder::create(collab, Some(context), folder_data);
  FolderTest {
    db,
    folder,
    cleaner,
    view_rx,
    section_rx: Some(section_rx),
  }
}

pub fn create_folder_with_workspace(uid: UserId, workspace_id: &str) -> FolderTest {
  create_folder(uid, workspace_id)
}

pub fn make_test_view(view_id: &str, parent_view_id: &str, belongings: Vec<String>) -> View {
  let belongings = belongings
    .into_iter()
    .map(ViewIdentifier::new)
    .collect::<Vec<ViewIdentifier>>();
  View {
    id: view_id.to_string(),
    parent_view_id: parent_view_id.to_string(),
    name: "".to_string(),
    children: RepeatedViewIdentifier::new(belongings),
    created_at: 0,
    is_favorite: false,
    layout: ViewLayout::Document,
    icon: None,
    created_by: None,
    last_edited_time: 0,
    last_edited_by: None,
    is_locked: None,
    extra: None,
  }
}

impl Deref for FolderTest {
  type Target = Folder;

  fn deref(&self) -> &Self::Target {
    &self.folder
  }
}

impl DerefMut for FolderTest {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.folder
  }
}

pub struct Cleaner(PathBuf);

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

#[allow(dead_code)]
pub fn setup_log() {
  static START: Once = Once::new();
  START.call_once(|| {
    let level = "info";
    let mut filters = vec![];
    filters.push(format!("collab_persistence={}", level));
    unsafe {
      std::env::set_var("RUST_LOG", filters.join(","));
    }

    let subscriber = Subscriber::builder()
      .with_env_filter(EnvFilter::from_default_env())
      .with_ansi(true)
      .finish();
    subscriber.try_init().unwrap();
  });
}
