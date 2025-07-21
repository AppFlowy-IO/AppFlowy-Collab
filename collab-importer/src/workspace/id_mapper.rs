use crate::workspace::entities::{DependencyType, WorkspaceRelationMap};
use collab_database::{database::get_row_document_id, rows::RowId};
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

      // if the dependency is database row document, we need to handle it differently
      if dependency.dependency_type == DependencyType::DatabaseRowDocument {
        let row_id = RowId::from(dependency.source_view_id.clone());
        let new_id = get_row_document_id(&row_id);
        if let Ok(new_id) = new_id {
          Self::overwrite_id(&mut id_map, &dependency.target_view_id, &new_id);
        }
      }
    }

    // workspace database meta
    if let Some(database_meta) = &relation_map.workspace_database_meta {
      for database_meta in database_meta {
        Self::map_id(&mut id_map, &database_meta.database_id);
        for view_id in &database_meta.view_ids {
          Self::map_id(&mut id_map, view_id);
        }
      }
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

  fn overwrite_id(map: &mut HashMap<String, String>, old_id: &str, new_id: &str) {
    map.insert(old_id.to_string(), new_id.to_string());
  }
}
