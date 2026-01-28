use collab::core::collab::{CollabOptions, default_client_id};
use collab::core::origin::CollabOrigin;
use collab::folder::{Folder, FolderData, UserId, Workspace, check_folder_is_valid};
use collab::preclude::Collab;
use uuid::Uuid;

#[test]
fn test_workspace_is_ready() {
  let uid = UserId::from(1);
  let object_id = "1";

  let workspace = Workspace::new(
    Uuid::new_v5(&Uuid::NAMESPACE_OID, "w1".as_bytes()),
    "".to_string(),
    uid.as_i64(),
  );
  let folder_data = FolderData::new(uid.as_i64(), workspace);
  let options = CollabOptions::new(
    Uuid::parse_str(object_id).unwrap_or_else(|_| Uuid::new_v4()),
    default_client_id(),
  );
  let collab = Collab::new_with_options(CollabOrigin::Empty, options).unwrap();
  let folder = Folder::create(collab, None, folder_data);

  let workspace_id = check_folder_is_valid(&folder.collab).unwrap();
  let expected_workspace_id = Uuid::new_v5(&Uuid::NAMESPACE_OID, "w1".as_bytes());
  assert_eq!(workspace_id, expected_workspace_id.to_string());
}

#[test]
fn validate_folder_data() {
  let options = CollabOptions::new(Uuid::new_v4(), default_client_id());
  let collab = Collab::new_with_options(CollabOrigin::Empty, options).unwrap();
  let result = Folder::open(collab, None);
  assert!(result.is_err());
}
