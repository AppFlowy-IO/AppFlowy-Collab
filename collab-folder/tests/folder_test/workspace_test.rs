use collab_folder::UserId;

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
