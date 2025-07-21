use std::fs::read_to_string;

use crate::workspace::entities::WorkspaceRelationMap;
use anyhow::{Result, anyhow};

pub struct RelationMapParser {}

impl RelationMapParser {
  pub async fn parse_relation_map(&self, relation_map_path: &str) -> Result<WorkspaceRelationMap> {
    let relation_map_content = read_to_string(relation_map_path)
      .map_err(|e| anyhow!(format!("failed to read relation_map.json: {}", e)))?;

    let relation_map: WorkspaceRelationMap = serde_json::from_str(&relation_map_content)
      .map_err(|e| anyhow!(format!("failed to parse relation_map.json: {}", e)))?;

    self.validate_relation_map(&relation_map)?;

    Ok(relation_map)
  }

  fn validate_relation_map(&self, relation_map: &WorkspaceRelationMap) -> Result<()> {
    if relation_map.workspace_id.to_string().is_empty() {
      return Err(anyhow!("workspace id must be non-empty"));
    }

    if relation_map.views.is_empty() {
      return Err(anyhow!("views must be non-empty"));
    }

    for (view_id, view_metadata) in &relation_map.views {
      if view_metadata.view_id != *view_id {
        return Err(anyhow!(format!(
          "view id mismatch: key={}, view.view_id={}",
          view_id, view_metadata.view_id
        )));
      }

      if let Some(parent_id) = &view_metadata.parent_id {
        // for the top level view, the parent id is the workspace id
        if !relation_map.views.contains_key(parent_id) && relation_map.workspace_id != *parent_id {
          return Err(anyhow!(format!(
            "view {} references non-existent parent {}",
            view_id, parent_id
          )));
        }
      }

      for child_id in &view_metadata.children {
        if !relation_map.views.contains_key(child_id) {
          return Err(anyhow!(format!(
            "view {} references non-existent child {}",
            view_id, child_id
          )));
        }
      }

      if let Some(_collab_metadata) = relation_map.collab_objects.get(view_id) {
        // if collab_metadata.object_id != view_metadata.collab_object_id {
        //   return Err(anyhow!(format!(
        //     "view {} collab_object_id mismatch: view references {}, collab_objects has {}",
        //     view_id, view_metadata.collab_object_id, collab_metadata.object_id
        //   )));
        // }
      } else {
        return Err(anyhow!(format!(
          "view {} has no corresponding entry in collab_objects",
          view_id
        )));
      }
    }

    Ok(())
  }
}
