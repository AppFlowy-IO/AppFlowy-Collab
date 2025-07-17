use collab_importer::workspace::WorkspaceRemapper;

#[tokio::test]
async fn test_workspace_remapper_creation() {
  let test_assets_path = "tests/asset/2025-07-16_22-15-54";

  let remapper = WorkspaceRemapper::new(test_assets_path.as_ref())
    .await
    .unwrap();

  let uid = 2368123586656;
  let device_id = "device_id";
  let workspace_name = "workspace_name";

  let folder = remapper
    .build_folder_collab(uid, device_id, workspace_name)
    .unwrap();
  let databases = remapper.build_database_collabs().await.unwrap();
  let documents = remapper.build_document_collabs().unwrap();

  assert!(!folder.get_workspace_id().unwrap().is_empty());
  assert_eq!(databases.len(), 1);
  assert_eq!(documents.len(), 6);
}

#[tokio::test]
async fn test_workspace_remapper_folder_structure() {
  let test_assets_path = "tests/asset/2025-07-16_22-15-54";

  let remapper = WorkspaceRemapper::new(test_assets_path.as_ref())
    .await
    .unwrap();

  let uid = 2368123586656;
  let device_id = "device_id";
  let workspace_name = "workspace_name";

  let folder = remapper
    .build_folder_collab(uid, device_id, workspace_name)
    .unwrap();

  let workspace_id = folder.get_workspace_id().unwrap();
  let workspace_info = folder.get_workspace_info(&workspace_id, uid).unwrap();

  assert_eq!(workspace_info.name, workspace_name);
  assert_eq!(workspace_info.id, workspace_id);

  let all_views = folder.get_all_views(uid);
  assert_eq!(all_views.len(), 8);
}

#[tokio::test]
async fn test_workspace_remapper_all_collabs() {
  let test_assets_path = "tests/asset/2025-07-16_22-15-54";

  let remapper = WorkspaceRemapper::new(test_assets_path.as_ref())
    .await
    .unwrap();

  let uid = 2368123586656;
  let device_id = "device_id";
  let workspace_name = "workspace_name";

  let workspace_collabs = remapper
    .build_all_collabs(uid, device_id, workspace_name)
    .await
    .unwrap();

  assert!(
    !workspace_collabs
      .folder
      .get_workspace_id()
      .unwrap()
      .is_empty()
  );
  assert_eq!(workspace_collabs.databases.len(), 1);
  assert_eq!(workspace_collabs.documents.len(), 6);
}
