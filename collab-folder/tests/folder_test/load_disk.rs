use collab::plugin_impl::disk::CollabDiskPlugin;
use collab::preclude::CollabBuilder;
use collab_folder::core::{Folder, FolderContext};
use collab_persistence::CollabDB;
use std::sync::Arc;

#[test]
fn load_from_disk() {
  let uid = 184965030800789504;
  let object_id = format!("{}:folder", uid);
  let db = Arc::new(CollabDB::open("/Users/weidongfu/Library/Containers/com.appflowy.macos/Data/Documents/flowy_dev/184983800244080640").unwrap());
  let mut collab = CollabBuilder::new(uid, &object_id).build();
  let disk_plugin = Arc::new(CollabDiskPlugin::new(uid, db).unwrap());
  collab.add_plugin(disk_plugin);
  collab.initial();

  let (view_tx, view_rx) = tokio::sync::broadcast::channel(100);
  let (trash_tx, trash_rx) = tokio::sync::broadcast::channel(100);
  let folder_context = FolderContext {
    view_change_tx: Some(view_tx),
    trash_change_tx: Some(trash_tx),
  };

  let folder = Folder::get_or_create(collab, folder_context);
  let json = folder.to_json();
  println!("{}", json);
}
