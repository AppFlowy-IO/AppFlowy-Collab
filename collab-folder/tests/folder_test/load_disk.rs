use fs_extra::file;
use nanoid::nanoid;
use std::path::Path;
use std::sync::Arc;
use walkdir::WalkDir;

use crate::util::setup_log;
use collab::plugin_impl::rocks_disk::RocksDiskPlugin;
use collab::preclude::CollabBuilder;
use collab_folder::core::{Folder, FolderContext};
use collab_persistence::kv::rocks_kv::{RocksCollabDB, RocksKVStore};
use collab_persistence::CollabDB;

#[test]
fn load_from_disk() {
  let uid: i64 = 185579439403307008;
  let source = "./tests/folder_test/dbs".to_string();

  let dest = format!("temp/{}", nanoid!());
  let dest_path = format!("{}/{}", source, dest);
  copy_folder_recursively(&source, &uid.to_string(), &dest).unwrap();

  let folder = create_folder_with_object_id(uid, &dest_path);
  let json = folder.to_json_value();
  println!("{}", json);

  // set current view
  folder.set_current_view("abc");
  let json = folder.to_json_value();
  println!("{}", json);
  drop(folder);

  // reopen
  let folder = create_folder_with_object_id(uid, &dest_path);
  let json = folder.to_json_value();
  println!("{}", json);

  std::fs::remove_dir_all(dest_path).unwrap();
}

fn copy_folder_recursively(
  parent_folder: &str,
  src_folder: &str,
  dest_folder: &str,
) -> std::io::Result<()> {
  let src_path = Path::new(parent_folder).join(src_folder);
  let dest_path = Path::new(parent_folder).join(dest_folder);

  for entry in WalkDir::new(&src_path) {
    let entry = entry?;
    let entry_path = entry.path();

    let relative_entry_path = entry_path.strip_prefix(&src_path).unwrap();
    let target_path = dest_path.join(relative_entry_path);

    if entry.file_type().is_dir() {
      std::fs::create_dir_all(target_path)?;
    } else {
      let options = file::CopyOptions::new().overwrite(true);
      file::copy(entry_path, target_path, &options).unwrap();
    }
  }

  Ok(())
}

fn create_folder_with_object_id(uid: i64, path: &str) -> Folder {
  setup_log();
  let object_id = format!("{}:folder", uid);
  let db = Arc::new(RocksCollabDB::open(path).unwrap());
  let mut collab = CollabBuilder::new(uid, &object_id).build();
  let disk_plugin = Arc::new(RocksDiskPlugin::new(uid, db).unwrap());
  collab.add_plugin(disk_plugin);
  collab.initial();

  let (view_tx, view_rx) = tokio::sync::broadcast::channel(100);
  let (trash_tx, trash_rx) = tokio::sync::broadcast::channel(100);
  let folder_context = FolderContext {
    view_change_tx: Some(view_tx),
    trash_change_tx: Some(trash_tx),
  };

  Folder::get_or_create(collab, folder_context)
}
