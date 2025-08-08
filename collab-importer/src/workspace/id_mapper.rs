use crate::workspace::entities::{DependencyType, WorkspaceRelationMap};
use collab_database::{database::get_row_document_id, rows::RowId};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Clone)]
pub struct IdMapper {
  pub id_map: HashMap<String, String>,
}

impl IdMapper {
  pub fn new(relation_map: &WorkspaceRelationMap) -> Self {
    let mut id_map = HashMap::new();

    // workspace id
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
      let mapped_source_view_id = Self::map_id(&mut id_map, &dependency.source_view_id);

      // ignore the file attachment for target view id
      if dependency.dependency_type == DependencyType::FileAttachment {
        continue;
      }

      let _mapped_target_view_id = Self::map_id(&mut id_map, &dependency.target_view_id);

      // if the dependency is database row document, we need to handle it differently
      if dependency.dependency_type == DependencyType::DatabaseRowDocument {
        let row_id = RowId::from(mapped_source_view_id.clone());
        let new_id = get_row_document_id(&row_id);
        if let Ok(new_id) = new_id {
          Self::overwrite_id(&mut id_map, &dependency.target_view_id, &new_id);
        }
      }
    }

    // workspace database meta
    
    for database_meta in &relation_map.workspace_database_meta {
      Self::map_id(&mut id_map, &database_meta.database_id);
      for view_id in &database_meta.view_ids {
        Self::map_id(&mut id_map, view_id);
      }
    }

    Self { id_map }
  }

  pub fn get_new_id(&self, old_id: &str) -> Option<&String> {
    self.id_map.get(old_id)
  }

  pub fn generate_new_id(&self) -> String {
    Uuid::new_v4().to_string()
  }

  fn map_id(map: &mut HashMap<String, String>, old_id: &str) -> String {
    let new_id = Uuid::new_v4().to_string();
    let new_id = map.entry(old_id.to_string()).or_insert(new_id);
    new_id.clone()
  }

  fn overwrite_id(map: &mut HashMap<String, String>, old_id: &str, new_id: &str) {
    map.insert(old_id.to_string(), new_id.to_string());
  }
}
