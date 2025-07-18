use collab_importer::workspace::id_mapper::IdMapper;
use collab_importer::workspace::relation_map_parser::RelationMapParser;
use std::collections::HashSet;

#[tokio::test]
async fn test_id_mapper() {
  let (_cleaner, unzip_path) = crate::util::sync_unzip_asset("2025-07-16_22-15-54").await.unwrap();
  let path = unzip_path.join("relation_map.json");
  let parser = RelationMapParser {};
  let relation_map = parser.parse_relation_map(&path.to_string_lossy()).await.unwrap();
  let id_mapper = IdMapper::new(&relation_map);

  let mut old_ids = HashSet::new();
  old_ids.insert(relation_map.workspace_id.clone());
  for (view_id, view) in &relation_map.views {
    old_ids.insert(view_id.clone());
    old_ids.insert(view.view_id.clone());
    if let Some(parent_id) = &view.parent_id {
      old_ids.insert(parent_id.clone());
    }
    for child_id in &view.children {
      old_ids.insert(child_id.clone());
    }
    old_ids.insert(view.collab_object_id.clone());
  }
  for (obj_id, obj) in &relation_map.collab_objects {
    old_ids.insert(obj_id.clone());
    old_ids.insert(obj.object_id.clone());
  }
  for dep in &relation_map.dependencies {
    old_ids.insert(dep.source_view_id.clone());
    old_ids.insert(dep.target_view_id.clone());
  }

  for old_id in &old_ids {
    assert!(
      id_mapper.get_new_id(old_id).is_some(),
      "id {} should be mapped",
      old_id
    );
  }

  for old_id in &old_ids {
    let new_id = id_mapper.get_new_id(old_id).unwrap();
    assert_ne!(old_id, new_id, "old and new IDs should be different");
  }

  assert_eq!(
    id_mapper.id_map.len(),
    old_ids.len(),
    "the number of mapped IDs should be equal to the number of unique old IDs"
  );

  let old_id_1 = "2f226c0f-51e7-4d04-9243-c61fb509b2e0";
  let new_id_1 = id_mapper.get_new_id(old_id_1).unwrap();

  let view = relation_map.views.get(old_id_1).unwrap();
  let new_id_2 = id_mapper.get_new_id(&view.view_id).unwrap();
  assert_eq!(
    new_id_1, new_id_2,
    "duplicated ids should map to the same new id"
  );

  let new_id_3 = id_mapper
    .get_new_id(&relation_map.dependencies[0].source_view_id)
    .unwrap();
  assert_eq!(
    new_id_1, new_id_3,
    "duplicated ids should map to the same new id"
  );
}
