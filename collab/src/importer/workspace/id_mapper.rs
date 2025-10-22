use crate::database::database::get_row_document_id;
use crate::importer::workspace::entities::{DependencyType, WorkspaceRelationMap};
use anyhow::{Result, anyhow};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Clone)]
pub struct IdMapper {
  pub id_map: HashMap<Uuid, Uuid>,
}

impl IdMapper {
  pub fn new(relation_map: &WorkspaceRelationMap) -> Result<Self> {
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
      let mapped_source_view_id = Self::map_string_id(&mut id_map, &dependency.source_view_id)?;

      // ignore the file attachment for target view id
      if dependency.dependency_type == DependencyType::FileAttachment {
        continue;
      }

      let _mapped_target_view_id = Self::map_string_id(&mut id_map, &dependency.target_view_id)?;

      // if the dependency is database row document, we need to handle it differently
      if dependency.dependency_type == DependencyType::DatabaseRowDocument {
        let new_id = get_row_document_id(&mapped_source_view_id);
        if let Ok(new_id) = new_id {
          Self::overwrite_id(&mut id_map, &dependency.target_view_id, &new_id)?;
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

    Ok(Self { id_map })
  }

  pub fn get_new_id(&self, old_id: &str) -> Option<Uuid> {
    let old_uuid = Uuid::parse_str(old_id).ok()?;
    Some(*self.id_map.get(&old_uuid)?)
  }

  pub fn get_new_id_from_uuid(&self, old_id: &Uuid) -> Option<Uuid> {
    Some(*self.id_map.get(old_id)?)
  }

  pub fn get_id_map_as_strings(&self) -> HashMap<String, String> {
    self
      .id_map
      .iter()
      .map(|(k, v)| (k.to_string(), v.to_string()))
      .collect()
  }

  pub fn generate_new_id(&self) -> String {
    Uuid::new_v4().to_string()
  }

  pub fn generate_new_uuid(&self) -> Uuid {
    Uuid::new_v4()
  }

  fn map_id(map: &mut HashMap<Uuid, Uuid>, old_id: &Uuid) -> Uuid {
    let new_id = Uuid::new_v4();
    *map.entry(*old_id).or_insert(new_id)
  }

  fn map_string_id(map: &mut HashMap<Uuid, Uuid>, old_id: &str) -> Result<Uuid> {
    let old_uuid =
      Uuid::parse_str(old_id).map_err(|e| anyhow!("Invalid UUID format '{}': {}", old_id, e))?;
    let new_id = Uuid::new_v4();
    Ok(*map.entry(old_uuid).or_insert(new_id))
  }

  fn overwrite_id(map: &mut HashMap<Uuid, Uuid>, old_id: &str, new_id: &str) -> Result<()> {
    let old_uuid = Uuid::parse_str(old_id)
      .map_err(|e| anyhow!("Invalid old UUID format '{}': {}", old_id, e))?;
    let new_uuid = Uuid::parse_str(new_id)
      .map_err(|e| anyhow!("Invalid new UUID format '{}': {}", new_id, e))?;
    map.insert(old_uuid, new_uuid);
    Ok(())
  }
}
