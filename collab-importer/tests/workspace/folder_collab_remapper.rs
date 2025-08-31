use crate::util::sync_unzip_asset;
use collab_folder::{Folder, ViewLayout, ViewId};
use uuid::Uuid;
use collab_importer::workspace::folder_collab_remapper::FolderCollabRemapper;
use collab_importer::workspace::id_mapper::IdMapper;
use collab_importer::workspace::relation_map_parser::RelationMapParser;

/// Helper function to parse string to ViewId for tests  
fn parse_view_id(s: &str) -> ViewId {
  Uuid::parse_str(s).expect(&format!("Invalid UUID format: {}", s))
}

#[allow(clippy::too_many_arguments)]
fn verify_view(
  folder: &Folder,
  id_mapper: &IdMapper,
  old_id: &str,
  expected_name: &str,
  expected_parent_id: &str,
  expected_children_len: usize,
  expected_layout: ViewLayout,
  uid: i64,
) {
  let new_id = id_mapper.get_new_id(old_id).unwrap();
  let view = folder.get_view(&parse_view_id(new_id), uid).unwrap();

  assert_eq!(view.name, expected_name);
  assert_eq!(
    view.parent_view_id,
    collab_entity::uuid_validation::view_id_from_any_string(expected_parent_id)
  );
  assert_eq!(view.children.len(), expected_children_len);
  assert_eq!(view.layout, expected_layout);
}

#[tokio::test]
async fn test_folder_collab_remapper() {
  let parser = RelationMapParser {};
  let (_cleaner, unzip_path) = sync_unzip_asset("2025-07-16_22-15-54").await.unwrap();
  let test_file_path = unzip_path.join("relation_map.json");

  let relation_map = parser
    .parse_relation_map(&test_file_path.to_string_lossy())
    .await
    .unwrap();
  let id_mapper = IdMapper::new(&relation_map);

  let uid = 123;

  let folder = FolderCollabRemapper::remap_to_folder_collab(
    &relation_map,
    &id_mapper,
    uid,
    "My Custom Workspace",
  )
  .unwrap();

  let workspace_id = folder.get_workspace_id().unwrap();
  assert_ne!(workspace_id, relation_map.workspace_id);
  assert_eq!(
    workspace_id,
    *id_mapper.get_new_id(&relation_map.workspace_id).unwrap()
  );

  let workspace_uuid = uuid::Uuid::parse_str(&workspace_id).unwrap();
  let workspace_info = folder.get_workspace_info(&workspace_uuid, uid).unwrap();
  assert_eq!(workspace_info.name, "My Custom Workspace");
  assert_eq!(
    workspace_info.id,
    uuid::Uuid::parse_str(&workspace_id).unwrap()
  );

  let top_level_views_count = relation_map
    .views
    .values()
    .filter(|view| {
      view
        .parent_id
        .as_ref()
        .is_none_or(|pid| pid == &relation_map.workspace_id)
    })
    .count();
  assert_eq!(workspace_info.child_views.len(), top_level_views_count);

  let all_views = folder.get_all_views(uid);
  // +1: workspace is also a view
  assert_eq!(all_views.len(), relation_map.views.len() + 1);

  for view in &all_views {
    if view.id.to_string() == workspace_id {
      continue;
    }

    let old_view_id = relation_map
      .views
      .keys()
      .find(|old_id| id_mapper.get_new_id(old_id) == Some(&view.id.to_string()))
      .expect("mapped view should exist in original relation map");

    let original_view = &relation_map.views[old_view_id];

    assert_eq!(view.name, original_view.name);
    assert_eq!(view.layout, original_view.layout);
    assert_eq!(view.icon, original_view.icon);
    assert_eq!(view.extra, original_view.extra);

    assert!(view.created_at > 0);
    assert!(view.last_edited_time > 0);

    if let Some(original_parent_id) = &original_view.parent_id {
      let expected_parent_id = id_mapper.get_new_id(original_parent_id).unwrap();
      assert_eq!(
        view.parent_view_id,
        collab_entity::uuid_validation::view_id_from_any_string(expected_parent_id)
      );
    } else {
      assert_eq!(
        view.parent_view_id,
        collab_entity::uuid_validation::view_id_from_any_string(&workspace_id)
      );
    }

    assert_eq!(view.children.len(), original_view.children.len());
    for (i, child) in view.children.iter().enumerate() {
      let expected_child_id = id_mapper.get_new_id(&original_view.children[i]).unwrap();
      assert_eq!(
        child.id,
        collab_entity::uuid_validation::view_id_from_any_string(expected_child_id)
      );
    }
  }
}

#[tokio::test]
async fn test_folder_hierarchy_structure() {
  let parser = RelationMapParser {};
  let (_cleaner, unzip_path) = sync_unzip_asset("2025-07-16_22-15-54").await.unwrap();
  let test_file_path = unzip_path.join("relation_map.json");

  let relation_map = parser
    .parse_relation_map(&test_file_path.to_string_lossy())
    .await
    .unwrap();
  let id_mapper = IdMapper::new(&relation_map);

  let uid = 456;

  let folder = FolderCollabRemapper::remap_to_folder_collab(
    &relation_map,
    &id_mapper,
    uid,
    "My Custom Workspace",
  )
  .unwrap();

  let workspace_id = folder.get_workspace_id().unwrap();

  let general_space_new_id = id_mapper
    .get_new_id("2f226c0f-51e7-4d04-9243-c61fb509b2e0")
    .unwrap();
  let getting_started_new_id = id_mapper
    .get_new_id("b8f96497-c880-4fea-8232-c31d57daab83")
    .unwrap();

  verify_view(
    &folder,
    &id_mapper,
    "2f226c0f-51e7-4d04-9243-c61fb509b2e0",
    "General",
    &workspace_id,
    2,
    ViewLayout::Document,
    uid,
  );
  verify_view(
    &folder,
    &id_mapper,
    "b8f96497-c880-4fea-8232-c31d57daab83",
    "Getting started",
    general_space_new_id,
    3,
    ViewLayout::Document,
    uid,
  );
  verify_view(
    &folder,
    &id_mapper,
    "6cbe3ff3-7b3a-4d3b-9eec-f0d1e0a8b8c3",
    "To-dos",
    general_space_new_id,
    0,
    ViewLayout::Board,
    uid,
  );
  verify_view(
    &folder,
    &id_mapper,
    "d0b0104e-996d-498b-b644-0556ebe6a37a",
    "Desktop guide",
    getting_started_new_id,
    0,
    ViewLayout::Document,
    uid,
  );
  verify_view(
    &folder,
    &id_mapper,
    "0a0fd09b-31ed-4cb6-814d-34280d65c5ef",
    "Mobile guide",
    getting_started_new_id,
    0,
    ViewLayout::Document,
    uid,
  );
  verify_view(
    &folder,
    &id_mapper,
    "b68f3000-6f31-452f-b781-db3a65aced1f",
    "Web guide",
    getting_started_new_id,
    0,
    ViewLayout::Document,
    uid,
  );
  verify_view(
    &folder,
    &id_mapper,
    "84baa901-8ebc-46e3-bad5-aaa29bd00830",
    "Shared",
    &workspace_id,
    0,
    ViewLayout::Document,
    uid,
  );

  let child_views = folder.get_views_belong_to(&parse_view_id(getting_started_new_id), uid);
  assert_eq!(child_views.len(), 3);
}
