use collab::core::collab::{CollabOptions, default_client_id};
use collab::core::origin::CollabOrigin;
use collab::preclude::Collab;
use collab_folder::{Folder, FolderData, UserId, Workspace, check_folder_is_valid};

#[test]
fn test_workspace_is_ready() {
  let uid = UserId::from(1);
  let object_id = "1";

  let workspace = Workspace::new("w1".to_string(), "".to_string(), uid.as_i64());
  let folder_data = FolderData::new(uid.as_i64(), workspace);
  let options = CollabOptions::new(object_id.to_string(), default_client_id());
  let collab = Collab::new_with_options(CollabOrigin::Empty, options).unwrap();
  let folder = Folder::create(collab, None, folder_data);

  let workspace_id = check_folder_is_valid(&folder.collab).unwrap();
  assert_eq!(workspace_id, "w1".to_string());
}

#[test]
fn validate_folder_data() {
  let options = CollabOptions::new("1".to_string(), default_client_id());
  let collab = Collab::new_with_options(CollabOrigin::Empty, options).unwrap();
  let result = Folder::open(collab, None);
  assert!(result.is_err());
}
