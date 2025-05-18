use collab::core::origin::CollabOrigin;
use collab::preclude::Collab;
use collab_folder::{Folder, FolderData, UserId, Workspace, check_folder_is_valid};

#[test]
fn test_workspace_is_ready() {
  let uid = UserId::from(1);
  let object_id = "1";

  let workspace = Workspace::new("w1".to_string(), "".to_string(), uid.as_i64());
  let folder_data = FolderData::new(workspace);
  let folder = Folder::create(
    uid,
    Collab::new_with_origin(CollabOrigin::Empty, object_id, vec![], true, None),
    None,
    folder_data,
  );

  let workspace_id = check_folder_is_valid(&folder.collab).unwrap();
  assert_eq!(workspace_id, "w1".to_string());
}

#[test]
fn validate_folder_data() {
  let collab = Collab::new_with_origin(CollabOrigin::Empty, "1", vec![], true, None);
  let result = Folder::open(1, collab, None);
  assert!(result.is_err());
}
