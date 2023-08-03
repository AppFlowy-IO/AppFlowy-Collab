use std::fs::{create_dir_all, File};
use std::io::copy;
use std::ops::Deref;
use std::path::Path;
use std::path::PathBuf;
use std::sync::{Arc, Once};

use collab::preclude::CollabBuilder;
use collab_folder::core::*;
use collab_persistence::kv::rocks_kv::RocksCollabDB;
use collab_plugins::local_storage::rocksdb::RocksdbDiskPlugin;
use nanoid::nanoid;
use tempfile::TempDir;
use tracing_subscriber::fmt::Subscriber;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;
use zip::read::ZipArchive;

pub struct FolderTest {
  folder: Folder,

  #[allow(dead_code)]
  db: Arc<RocksCollabDB>,

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
  create_folder_with_data(id, None)
}

pub fn create_folder_with_data(id: &str, folder_data: Option<FolderData>) -> FolderTest {
  let uid = 1;
  let tempdir = TempDir::new().unwrap();

  let path = tempdir.into_path();
  let db = Arc::new(RocksCollabDB::open(path.clone()).unwrap());
  let disk_plugin = RocksdbDiskPlugin::new(uid, Arc::downgrade(&db));
  let cleaner: Cleaner = Cleaner::new(path);

  let collab = CollabBuilder::new(1, id)
    .with_plugin(disk_plugin)
    .build()
    .unwrap();
  collab.lock().initialize();

  let (view_tx, view_rx) = tokio::sync::broadcast::channel(100);
  let (trash_tx, trash_rx) = tokio::sync::broadcast::channel(100);
  let context = FolderNotify {
    view_change_tx: view_tx,
    trash_change_tx: trash_tx,
  };
  let folder = Folder::create(Arc::new(collab), Some(context), folder_data);
  FolderTest {
    db,
    folder,
    cleaner,
    view_rx,
    trash_rx: Some(trash_rx),
  }
}

pub fn open_folder_with_db(uid: i64, object_id: &str, db_path: PathBuf) -> FolderTest {
  let db = Arc::new(RocksCollabDB::open(db_path.clone()).unwrap());
  let disk_plugin = RocksdbDiskPlugin::new(uid, Arc::downgrade(&db));
  let cleaner: Cleaner = Cleaner::new(db_path);
  let collab = CollabBuilder::new(1, object_id)
    .with_plugin(disk_plugin)
    .build()
    .unwrap();
  collab.lock().initialize();

  let (view_tx, view_rx) = tokio::sync::broadcast::channel(100);
  let (trash_tx, trash_rx) = tokio::sync::broadcast::channel(100);
  let context = FolderNotify {
    view_change_tx: view_tx,
    trash_change_tx: trash_tx,
  };
  let folder = Folder::open(Arc::new(collab), Some(context));
  FolderTest {
    folder,
    db,
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
    child_views: Default::default(),
    created_at: 123,
  };

  test.folder.workspaces.create_workspace(workspace);
  test.folder.set_current_workspace(workspace_id);
  test
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
    desc: "".to_string(),
    children: RepeatedViewIdentifier::new(belongings),
    created_at: 0,
    layout: ViewLayout::Document,
    is_favorite: false,
    icon: None,
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

pub fn gen_view_icon(ty: String, value: String) -> Option<ViewIcon> {
  let ty = IconType::from_string(&ty)?;
  Some(ViewIcon { ty, value })
}

pub fn unzip_history_folder_db(folder_name: &str) -> std::io::Result<PathBuf> {
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
  Ok(PathBuf::from(path))
}
