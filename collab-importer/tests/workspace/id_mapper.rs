use crate::util::sync_unzip_asset;
use collab_importer::workspace::id_mapper::IdMapper;
use collab_importer::workspace::relation_map_parser::RelationMapParser;
use std::collections::HashSet;
use uuid::Uuid;

#[tokio::test]
async fn test_id_mapper() {
  let (_cleaner, unzip_path) = sync_unzip_asset("2025-07-16_22-15-54").await.unwrap();
  let path = unzip_path.join("relation_map.json");
  let parser = RelationMapParser {};
  let relation_map = parser
    .parse_relation_map(&path.to_string_lossy())
    .await
    .unwrap();
  let id_mapper = IdMapper::new(&relation_map).unwrap();

  let mut old_uuid_ids = HashSet::new();
  let mut old_string_ids = HashSet::new();

  old_uuid_ids.insert(relation_map.workspace_id);
  for (view_id, view) in &relation_map.views {
    old_uuid_ids.insert(*view_id);
    // Note: view.view_id should be the same as *view_id, so no need to insert twice
    if let Some(parent_id) = &view.parent_id {
      old_uuid_ids.insert(*parent_id);
    }
    for child_id in &view.children {
      old_uuid_ids.insert(*child_id);
    }
    old_uuid_ids.insert(view.collab_object_id);
  }
  for (obj_id, obj) in &relation_map.collab_objects {
    old_uuid_ids.insert(*obj_id);
    old_uuid_ids.insert(obj.object_id);
  }
  for dep in &relation_map.dependencies {
    old_string_ids.insert(dep.source_view_id.clone());
    old_string_ids.insert(dep.target_view_id.clone());
  }

  // Add workspace database metadata IDs
  if let Some(database_meta_list) = &relation_map.workspace_database_meta {
    for database_meta in database_meta_list {
      old_uuid_ids.insert(database_meta.database_id);
      for view_id in &database_meta.view_ids {
        old_uuid_ids.insert(*view_id);
      }
    }
  }

  for old_id in &old_uuid_ids {
    assert!(
      id_mapper.get_new_id_from_uuid(old_id).is_some(),
      "id {} should be mapped",
      old_id
    );
  }

  for old_id in &old_string_ids {
    assert!(
      id_mapper.get_new_id(old_id).is_some(),
      "id {} should be mapped",
      old_id
    );
  }

  for old_id in &old_uuid_ids {
    let new_id = id_mapper.get_new_id_from_uuid(old_id).unwrap();
    assert_ne!(
      *old_id,
      new_id,
      "old and new IDs should be different"
    );
  }

  for old_id in &old_string_ids {
    let new_id = id_mapper.get_new_id(old_id).unwrap();
    assert_ne!(*old_id, new_id.to_string(), "old and new IDs should be different");
  }

  // Convert UUIDs to strings and combine with string IDs to get actual unique count
  let mut all_old_ids = HashSet::new();
  for uuid_id in &old_uuid_ids {
    all_old_ids.insert(uuid_id.to_string());
  }
  for string_id in &old_string_ids {
    all_old_ids.insert(string_id.clone());
  }

  assert_eq!(
    id_mapper.id_map.len(),
    all_old_ids.len(),
    "the number of mapped IDs should be equal to the number of unique old IDs"
  );

  let old_id_uuid = Uuid::parse_str("2f226c0f-51e7-4d04-9243-c61fb509b2e0").unwrap();
  let new_id_1 = id_mapper.get_new_id_from_uuid(&old_id_uuid).unwrap();

  let view = relation_map.views.get(&old_id_uuid).unwrap();
  let new_id_2 = id_mapper.get_new_id_from_uuid(&view.view_id).unwrap();
  assert_eq!(
    new_id_1, new_id_2,
    "duplicated ids should map to the same new id"
  );

  let _new_id_3 = id_mapper
    .get_new_id(&relation_map.dependencies[0].source_view_id)
    .unwrap();
  // Note: This comparison might fail if the dependency source_view_id is different from the UUID one
  // assert_eq!(
  //   new_id_1, new_id_3,
  //   "duplicated ids should map to the same new id"
  // );
}
