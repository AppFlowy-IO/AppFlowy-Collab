use crate::util::sync_unzip_asset;
use collab_importer::workspace::WorkspaceRemapper;

#[tokio::test]
async fn test_space_view_edge_case_handler() {
  let (_cleaner, unzip_path) = sync_unzip_asset("My Workspace_2025-07-23_21-52-13")
    .await
    .unwrap();
  let test_assets_path = unzip_path;

  // Use a valid UUID for workspace ID
  let custom_workspace_id = uuid::Uuid::new_v4().to_string();
  let remapper =
    WorkspaceRemapper::new(test_assets_path.as_ref(), Some(custom_workspace_id.clone()))
      .await
      .unwrap();

  let relation_map = remapper.get_relation_map();

  let mut found_space = false;
  let mut space_view_id = None;

  for (view_id, view_metadata) in &relation_map.views {
    if let Some(extra) = &view_metadata.extra {
      if let Ok(space_info) = serde_json::from_str::<serde_json::Value>(extra) {
        if let Some(is_space) = space_info.get("is_space") {
          if is_space.as_bool() == Some(true) {
            found_space = true;
            space_view_id = Some(view_id.clone());
            assert_eq!(view_metadata.name, "General");
            assert_eq!(space_info["space_icon"], "interface_essential/home-3");
            assert_eq!(space_info["space_icon_color"], "0xFFA34AFD");
            assert_eq!(space_info["space_permission"], 0);
            break;
          }
        }
      }
    }
  }

  assert!(found_space, "no space view was created");

  let space_id = space_view_id.expect("space view should exist");
  let workspace_id = &relation_map.workspace_id;

  for (view_id, view_metadata) in &relation_map.views {
    if view_id != &space_id {
      if let Some(parent_id) = &view_metadata.parent_id {
        assert_ne!(
          parent_id, workspace_id,
          "view {} should not be directly parented to workspace",
          view_id
        );
      }
    }
  }

  let space_document_path = test_assets_path
    .join("collab_jsons")
    .join("documents")
    .join(format!("{}.json", space_id));
  assert!(
    space_document_path.exists(),
    "space document should be created"
  );

  let document_content = std::fs::read_to_string(&space_document_path).unwrap();
  let document_json: serde_json::Value = serde_json::from_str(&document_content).unwrap();

  assert!(document_json.get("document").is_some());
  let document = &document_json["document"];
  assert_eq!(document["page_id"], space_id.to_string());

  let id_mapping = remapper.get_id_mapping();
  assert!(
    id_mapping.contains_key(&space_id),
    "space view id should be in id mapping"
  );

  let uid = 123456789;
  let workspace_name = "test_workspace";
  let folder = remapper.build_folder_collab(uid, workspace_name).unwrap();

  let folder_workspace_id = folder.get_workspace_id().unwrap();
  let expected_workspace_id = uuid::Uuid::parse_str(&custom_workspace_id).unwrap();
  assert_eq!(
    folder_workspace_id,
    expected_workspace_id.to_string(),
    "folder should use custom workspace id"
  );

  let all_views = folder.get_all_views(uid);
  let space_view_found = all_views.iter().any(|view| {
    if let Some(extra) = &view.extra {
      if let Ok(space_info) = serde_json::from_str::<serde_json::Value>(extra) {
        space_info.get("is_space").and_then(|v| v.as_bool()) == Some(true)
      } else {
        false
      }
    } else {
      false
    }
  });
  assert!(
    space_view_found,
    "space view should be present in folder collab"
  );
}
