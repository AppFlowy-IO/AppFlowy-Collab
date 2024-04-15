use collab::core::collab::MutexCollab;
use collab::core::origin::CollabOrigin;
use collab::preclude::Collab;
use collab_folder::{check_folder_is_valid, Folder, FolderData, UserId, Workspace};
use std::sync::Arc;

use crate::util::create_folder;

#[tokio::test]
async fn update_workspace_test() {
  let uid = UserId::from(1);
  let folder_test = create_folder(uid, "1").await;
  let workspace = folder_test.get_current_workspace().unwrap();
  assert_eq!(workspace.name, "");

  folder_test.update_workspace("My first workspace");
  let workspace = folder_test.get_current_workspace().unwrap();
  assert_eq!(workspace.name, "My first workspace");
}

#[tokio::test]
async fn test_workspace_is_ready() {
  let uid = UserId::from(1);
  let object_id = "1";

  let workspace = Workspace::new("w1".to_string(), "".to_string(), uid.as_i64());
  let folder_data = FolderData::new(workspace);
  let collab = Arc::new(MutexCollab::new(Collab::new_with_origin(
    CollabOrigin::Empty,
    object_id,
    vec![],
    true,
  )));
  let _ = Folder::create(uid, collab.clone(), None, folder_data);

  let workspace_id = check_folder_is_valid(&collab.lock()).unwrap();
  assert_eq!(workspace_id, "w1".to_string());
}

#[tokio::test]
async fn validate_folder_data() {
  let collab = Arc::new(MutexCollab::new(Collab::new_with_origin(
    CollabOrigin::Empty,
    "1",
    vec![],
    true,
  )));
  assert!(Folder::validate(&collab.lock()).is_err());

  let workspace = Workspace::new("w1".to_string(), "".to_string(), 1);
  let folder_data = FolderData::new(workspace);
  let _ = Folder::create(1, collab.clone(), None, folder_data);
  assert!(Folder::validate(&collab.lock()).is_ok());
}
