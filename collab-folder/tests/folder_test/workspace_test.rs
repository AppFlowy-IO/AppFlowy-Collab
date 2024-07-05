use collab::core::origin::CollabOrigin;
use collab::preclude::Collab;
use collab_folder::{check_folder_is_valid, Folder, FolderData, UserId, Workspace};
use std::sync::{Arc, Mutex};

#[test]
fn test_workspace_is_ready() {
  let uid = UserId::from(1);
  let object_id = "1";

  let workspace = Workspace::new("w1".to_string(), "".to_string(), uid.as_i64());
  let folder_data = FolderData::new(workspace);
  let collab = Arc::new(Mutex::new(Collab::new_with_origin(
    CollabOrigin::Empty,
    object_id,
    vec![],
    true,
  )));
  let _ = Folder::create(uid, collab.clone(), None, folder_data);

  let workspace_id = check_folder_is_valid(&collab.lock().unwrap()).unwrap();
  assert_eq!(workspace_id, "w1".to_string());
}

#[test]
fn validate_folder_data() {
  let collab = Collab::new_with_origin(CollabOrigin::Empty, "1", vec![], true);
  assert!(Folder::validate(&collab).is_err());

  let workspace = Workspace::new("w1".to_string(), "".to_string(), 1);
  let folder_data = FolderData::new(workspace);
  let collab = Arc::new(Mutex::new(collab));
  let _ = Folder::create(1, collab.clone(), None, folder_data);
  assert!(Folder::validate(&collab.lock().unwrap()).is_ok());
}
