use crate::workspace::entities::WorkspaceRelationMap;
use std::collections::HashMap;
use uuid::Uuid;

pub struct IdMapper {
  pub id_map: HashMap<String, String>,
}

impl IdMapper {
  pub fn new(relation_map: &WorkspaceRelationMap) -> Self {
    let mut id_map = HashMap::new();

    // workspace ID
    Self::map_id(&mut id_map, &relation_map.workspace_id);

    // views
    for (view_id, view_metadata) in &relation_map.views {
      Self::map_id(&mut id_map, view_id);
      Self::map_id(&mut id_map, &view_metadata.view_id);
      if let Some(parent_id) = &view_metadata.parent_id {
        Self::map_id(&mut id_map, parent_id);
      }
      for child_id in &view_metadata.children {
        Self::map_id(&mut id_map, child_id);
      }
      Self::map_id(&mut id_map, &view_metadata.collab_object_id);
    }

    // collab objects
    for (view_id, collab_metadata) in &relation_map.collab_objects {
      Self::map_id(&mut id_map, view_id);
      Self::map_id(&mut id_map, &collab_metadata.object_id);
    }

    // dependencies
    for dependency in &relation_map.dependencies {
      Self::map_id(&mut id_map, &dependency.source_view_id);
      Self::map_id(&mut id_map, &dependency.target_view_id);
    }

    Self { id_map }
  }

  pub fn get_new_id(&self, old_id: &str) -> Option<&String> {
    self.id_map.get(old_id)
  }

  fn map_id(map: &mut HashMap<String, String>, old_id: &str) {
    map
      .entry(old_id.to_string())
      .or_insert_with(|| Uuid::new_v4().to_string());
  }
}
