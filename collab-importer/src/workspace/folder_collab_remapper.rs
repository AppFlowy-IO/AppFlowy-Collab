use anyhow::{Result, anyhow};
use collab::core::collab::{CollabOptions, default_client_id};
use collab::preclude::Collab;
use collab_folder::{
  CollabOrigin, Folder, RepeatedViewIdentifier, View, ViewIdentifier, Workspace,
  default_folder_data, timestamp,
};
use uuid::Uuid;

use crate::workspace::entities::WorkspaceRelationMap;
use crate::workspace::id_mapper::IdMapper;

pub struct FolderCollabRemapper;

impl FolderCollabRemapper {
  pub fn remap_to_folder_collab(
    relation_map: &WorkspaceRelationMap,
    id_mapper: &IdMapper,
    uid: i64,
    workspace_name: &str,
  ) -> Result<Folder> {
    let new_workspace_id = id_mapper
      .get_new_id(&relation_map.workspace_id)
      .ok_or_else(|| anyhow!("missing mapping for workspace id"))?
      .clone();

    let current_time = timestamp();

    let mut folder_data = default_folder_data(uid, &new_workspace_id);
    let mut views = vec![];
    let mut top_level_view_ids = vec![];

    for (old_view_id, view_metadata) in &relation_map.views {
      let new_view_id = id_mapper
        .get_new_id(old_view_id)
        .ok_or_else(|| anyhow!("missing mapping for view id: {}", old_view_id))?
        .clone();

      let new_parent_id = if let Some(old_parent_id) = &view_metadata.parent_id {
        id_mapper
          .get_new_id(old_parent_id)
          .ok_or_else(|| anyhow!("missing mapping for parent id: {}", old_parent_id))?
          .clone()
      } else {
        new_workspace_id.clone()
      };

      if view_metadata
        .parent_id
        .as_ref()
        .is_none_or(|pid| pid == &relation_map.workspace_id)
      {
        top_level_view_ids.push(ViewIdentifier::new(
          collab_entity::uuid_validation::view_id_from_any_string(&new_view_id),
        ));
      }

      let children_ids: Vec<ViewIdentifier> = view_metadata
        .children
        .iter()
        .filter_map(|child_id| {
          id_mapper.get_new_id(child_id).map(|new_id| {
            ViewIdentifier::new(collab_entity::uuid_validation::view_id_from_any_string(
              new_id,
            ))
          })
        })
        .collect();

      let mut view = View::new(
        new_view_id.clone(),
        new_parent_id,
        view_metadata.name.clone(),
        view_metadata.layout.clone(),
        Some(uid),
      );

      view.created_at = current_time;
      view.last_edited_time = current_time;
      view.children = RepeatedViewIdentifier::new(children_ids);
      view.extra = view_metadata.extra.clone();
      view.icon = view_metadata.icon.clone();
      views.push(view);
    }

    folder_data.views = views;
    folder_data.workspace = Workspace {
      id: Uuid::parse_str(&new_workspace_id).map_err(|_| anyhow!("invalid workspace id"))?,
      name: workspace_name.to_string(),
      child_views: RepeatedViewIdentifier::new(top_level_view_ids),
      created_at: current_time,
      created_by: Some(uid),
      last_edited_time: current_time,
      last_edited_by: Some(uid),
    };

    let workspace_uuid = Uuid::parse_str(&new_workspace_id).unwrap_or_else(|_| Uuid::new_v4());
    let options = CollabOptions::new(workspace_uuid, default_client_id());
    let collab = Collab::new_with_options(CollabOrigin::Empty, options).unwrap();
    let folder = Folder::create(collab, None, folder_data);
    Ok(folder)
  }
}
