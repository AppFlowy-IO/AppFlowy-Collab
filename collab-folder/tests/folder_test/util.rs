use std::fs::{create_dir_all, File};
use std::io::copy;
use std::ops::{Deref, DerefMut};
use std::path::Path;
use std::path::PathBuf;
use std::sync::{Arc, Once};

use collab::core::collab::DataSource;
use collab::preclude::CollabBuilder;
use collab_entity::CollabType;
use collab_folder::*;
use collab_plugins::local_storage::rocksdb::rocksdb_plugin::RocksdbDiskPlugin;
use collab_plugins::local_storage::rocksdb::util::KVDBCollabPersistenceImpl;
use collab_plugins::CollabKVDB;
use nanoid::nanoid;
use tempfile::TempDir;
use tracing_subscriber::fmt::Subscriber;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;
use zip::read::ZipArchive;

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
  let folder_data = FolderData::new(workspace);
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

  let mut collab = CollabBuilder::new(uid.as_i64(), workspace_id, DataSource::Disk(None))
    .with_plugin(disk_plugin)
    .with_device_id("1")
    .build()
    .unwrap();
  collab.initialize();

  let (view_tx, view_rx) = tokio::sync::broadcast::channel(100);
  let (section_tx, section_rx) = tokio::sync::broadcast::channel(100);
  let context = FolderNotify {
    view_change_tx: view_tx,
    section_change_tx: section_tx,
  };
  let folder = Folder::create(uid, collab, Some(context), folder_data);
  FolderTest {
    db,
    folder,
    cleaner,
    view_rx,
    section_rx: Some(section_rx),
  }
}

pub fn open_folder_with_db(
  uid: UserId,
  workspace_id: &str,
  object_id: &str,
  db_path: PathBuf,
) -> FolderTest {
  let db = Arc::new(CollabKVDB::open(db_path.clone()).unwrap());
  let disk_plugin = Box::new(RocksdbDiskPlugin::new(
    uid.as_i64(),
    workspace_id.to_string(),
    object_id.to_string(),
    CollabType::Folder,
    Arc::downgrade(&db),
  ));
  let data_source = KVDBCollabPersistenceImpl {
    db: Arc::downgrade(&db),
    uid: uid.as_i64(),
    workspace_id: workspace_id.to_string(),
  };
  let cleaner: Cleaner = Cleaner::new(db_path);
  let mut collab = CollabBuilder::new(1, object_id, data_source.into())
    .with_device_id("1")
    .with_plugin(disk_plugin)
    .build()
    .unwrap();

  collab.initialize();

  let (view_tx, view_rx) = tokio::sync::broadcast::channel(100);
  let (section_tx, section_rx) = tokio::sync::broadcast::channel(100);
  let context = FolderNotify {
    view_change_tx: view_tx,
    section_change_tx: section_tx,
  };
  let folder = Folder::open(uid, collab, Some(context)).unwrap();
  FolderTest {
    folder,
    db,
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
    std::env::set_var("RUST_LOG", filters.join(","));

    let subscriber = Subscriber::builder()
      .with_env_filter(EnvFilter::from_default_env())
      .with_ansi(true)
      .finish();
    subscriber.try_init().unwrap();
  });
}

pub fn unzip_history_folder_db(folder_name: &str) -> std::io::Result<(Cleaner, PathBuf)> {
  // Open the zip file
  let zip_file_path = format!("./tests/folder_test/history_folder/{}.zip", folder_name);
  let reader = File::open(zip_file_path)?;
  let output_folder_path = format!(
    "./tests/folder_test/history_folder/unit_test_{}",
    nanoid!(6)
  );

  // Create a ZipArchive from the file
  let mut archive = ZipArchive::new(reader)?;

  // Iterate through each file in the zip
  for i in 0..archive.len() {
    let mut file = archive.by_index(i)?;
    let outpath = Path::new(&output_folder_path).join(file.mangled_name());

    if file.name().ends_with('/') {
      // Create directory
      create_dir_all(&outpath)?;
    } else {
      // Write file
      if let Some(p) = outpath.parent() {
        if !p.exists() {
          create_dir_all(p)?;
        }
      }
      let mut outfile = File::create(&outpath)?;
      copy(&mut file, &mut outfile)?;
    }
  }
  let path = format!("{}/{}", output_folder_path, folder_name);
  Ok((
    Cleaner::new(PathBuf::from(output_folder_path)),
    PathBuf::from(path),
  ))
}
