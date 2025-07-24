use crate::util::sync_unzip_asset;
use collab_importer::workspace::WorkspaceRemapper;

#[tokio::test]
async fn test_workspace_remapper_creation() {
  let (_cleaner, unzip_path) = sync_unzip_asset("2025-07-16_22-15-54").await.unwrap();
  let test_assets_path = unzip_path;

  let remapper = WorkspaceRemapper::new(test_assets_path.as_ref(), None)
    .await
    .unwrap();

  let uid = 2368123586656;
  let workspace_name = "workspace_name";

  let folder = remapper.build_folder_collab(uid, workspace_name).unwrap();
  let databases = remapper.build_database_collabs().await.unwrap();
  let documents = remapper.build_document_collabs().unwrap();
  let row_documents = remapper.build_row_document_collabs().unwrap();

  assert!(!folder.get_workspace_id().unwrap().is_empty());
  assert_eq!(databases.len(), 1);
  assert_eq!(documents.len(), 6);
  assert_eq!(row_documents.len(), 0);
}

#[tokio::test]
async fn test_workspace_remapper_folder_structure() {
  let (_cleaner, unzip_path) = sync_unzip_asset("2025-07-16_22-15-54").await.unwrap();
  let test_assets_path = unzip_path;

  let remapper = WorkspaceRemapper::new(test_assets_path.as_ref(), None)
    .await
    .unwrap();

  let uid = 2368123586656;
  let workspace_name = "workspace_name";

  let folder = remapper.build_folder_collab(uid, workspace_name).unwrap();

  let workspace_id = folder.get_workspace_id().unwrap();
  let workspace_info = folder.get_workspace_info(&workspace_id, uid).unwrap();

  assert_eq!(workspace_info.name, workspace_name);
  assert_eq!(workspace_info.id, workspace_id);

  let all_views = folder.get_all_views(uid);
  assert_eq!(all_views.len(), 8);
}

#[tokio::test]
async fn test_workspace_remapper_all_collabs() {
  let (_cleaner, unzip_path) = sync_unzip_asset("2025-07-16_22-15-54").await.unwrap();
  let test_assets_path = unzip_path;

  let remapper = WorkspaceRemapper::new(test_assets_path.as_ref(), None)
    .await
    .unwrap();

  let uid = 2368123586656;
  let workspace_name = "workspace_name";

  let workspace_collabs = remapper
    .build_all_collabs(uid, workspace_name, "workspace_database_storage_id")
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
  assert_eq!(workspace_collabs.row_documents.len(), 0);
}

#[tokio::test]
async fn test_workspace_remapper_row_document_collabs() {
  let (_cleaner, unzip_path) = sync_unzip_asset("2025-07-18_15-31-18").await.unwrap();
  let test_assets_path = unzip_path;

  let remapper = WorkspaceRemapper::new(test_assets_path.as_ref(), None)
    .await
    .unwrap();

  let uid = 2368123586656;
  let workspace_name = "workspace_name";

  let folder = remapper.build_folder_collab(uid, workspace_name).unwrap();
  let databases = remapper.build_database_collabs().await.unwrap();
  let row_documents = remapper.build_row_document_collabs().unwrap();

  assert!(!folder.get_workspace_id().unwrap().is_empty());
  assert_eq!(databases.len(), 1);
  assert_eq!(row_documents.len(), 1);

  let row_document = &row_documents[0];
  assert!(!row_document.object_id().is_empty());

  let original_row_doc_id = "3edeba80-8862-54b6-bf1b-8d868dad3e0c";
  assert_ne!(row_document.object_id(), original_row_doc_id);
}
