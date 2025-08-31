use crate::workspace::entities::{ViewMetadata, WorkspaceRelationMap};
use crate::workspace::id_mapper::IdMapper;
use anyhow::Result;
use collab_database::database::timestamp;
use collab_folder::ViewLayout;
use serde_json;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use uuid::Uuid;

pub struct SpaceViewEdgeCaseHandler {
  id_mapper: Arc<IdMapper>,
  original_workspace_id: Uuid,
}

impl SpaceViewEdgeCaseHandler {
  pub fn new(id_mapper: Arc<IdMapper>, original_workspace_id: Uuid) -> Self {
    Self {
      id_mapper,
      original_workspace_id,
    }
  }

  pub fn handle_missing_space_view(
    &self,
    relation_map: &mut WorkspaceRelationMap,
    export_path: &Path,
    id_mapper: &mut IdMapper,
  ) -> Result<Option<String>> {
    if self.has_space_views(relation_map) {
      return Ok(None);
    }

    let space_view_uuid = self.id_mapper.generate_new_uuid();
    let space_view = self.create_default_space_view(&space_view_uuid)?;
    relation_map.views.insert(space_view_uuid, space_view);

    id_mapper
      .id_map
      .insert(space_view_uuid, space_view_uuid);
    self.reparent_workspace_views(relation_map, &space_view_uuid)?;
    self.generate_space_document(&space_view_uuid, export_path)?;

    Ok(Some(space_view_uuid.to_string()))
  }

  fn has_space_views(&self, relation_map: &WorkspaceRelationMap) -> bool {
    for view_metadata in relation_map.views.values() {
      if let Some(extra) = &view_metadata.extra {
        if let Ok(space_info) = serde_json::from_str::<serde_json::Value>(extra) {
          if let Some(is_space) = space_info.get("is_space") {
            if is_space.as_bool() == Some(true) {
              return true;
            }
          }
        }
      }
    }
    false
  }

  fn create_default_space_view(&self, space_view_uuid: &Uuid) -> Result<ViewMetadata> {
    let current_time = timestamp();

    let space_info = serde_json::json!({
        "is_space": true,
        "space_permission": 0,
        "space_created_at": current_time,
        "space_icon": "interface_essential/home-3",
        "space_icon_color": "0xFFA34AFD"
    });

    let space_view = ViewMetadata {
      view_id: *space_view_uuid,
      name: "General".to_string(),
      layout: ViewLayout::Document,
      parent_id: Some(self.original_workspace_id),
      children: Vec::new(),
      collab_object_id: *space_view_uuid,
      created_at: current_time,
      updated_at: current_time,
      extra: Some(space_info.to_string()),
      icon: None,
    };

    Ok(space_view)
  }

  fn reparent_workspace_views(
    &self,
    relation_map: &mut WorkspaceRelationMap,
    space_view_id: &Uuid,
  ) -> Result<()> {
    let mut workspace_children = Vec::new();

    let space_view_uuid = *space_view_id;
    let original_workspace_uuid = self.original_workspace_id;

    for (view_id, view_metadata) in relation_map.views.iter_mut() {
      if view_id != &space_view_uuid {
        if let Some(parent_id) = &view_metadata.parent_id {
          if parent_id == &original_workspace_uuid {
            view_metadata.parent_id = Some(space_view_uuid);
            workspace_children.push(*view_id);
          }
        }
      }
    }

    if let Some(space_view) = relation_map.views.get_mut(&space_view_uuid) {
      space_view.children = workspace_children;
    }
    Ok(())
  }

  fn generate_space_document(&self, space_view_id: &Uuid, export_path: &Path) -> Result<()> {
    let documents_dir = export_path.join("collab_jsons").join("documents");
    fs::create_dir_all(&documents_dir)?;

    let document_path = documents_dir.join(format!("{}.json", space_view_id));

    let document_content = serde_json::json!({
        "document": {
            "page_id": space_view_id.to_string(),
            "blocks": {},
            "meta": {
                "children_map": {},
                "text_map": {}
            }
        }
    });

    fs::write(
      &document_path,
      serde_json::to_string_pretty(&document_content)?,
    )?;

    Ok(())
  }
}
