use anyhow::{Result, anyhow};
use collab_database::database::Database;
use collab_document::document::Document;
use collab_entity::CollabType;
use collab_folder::Folder;
use serde_json;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::workspace::database_collab_remapper::DatabaseCollabRemapper;
use crate::workspace::document_collab_remapper::DocumentCollabRemapper;
use crate::workspace::entities::WorkspaceRelationMap;
use crate::workspace::folder_collab_remapper::FolderCollabRemapper;
use crate::workspace::id_mapper::IdMapper;
use crate::workspace::relation_map_parser::RelationMapParser;

pub struct WorkspaceRemapper {
  relation_map: WorkspaceRelationMap,
  id_mapping: HashMap<String, String>,
  workspace_path: PathBuf,
}

pub struct WorkspaceCollabs {
  pub folder: Folder,
  pub databases: Vec<Database>,
  pub documents: Vec<Document>,
}

impl WorkspaceRemapper {
  pub async fn new(workspace_path: &Path) -> Result<Self> {
    if !workspace_path.exists() {
      return Err(anyhow!(
        "workspace path does not exist: {}",
        workspace_path.display()
      ));
    }

    let relation_map_path = workspace_path.join("relation_map.json");
    if !relation_map_path.exists() {
      return Err(anyhow!(
        "relation_map.json not found at: {}",
        relation_map_path.display()
      ));
    }

    let parser = RelationMapParser {};
    let relation_map = parser
      .parse_relation_map(&relation_map_path.to_string_lossy())
      .await
      .map_err(|e| anyhow!("failed to parse relation map: {}", e))?;

    let id_mapper = IdMapper::new(&relation_map);
    let id_mapping = id_mapper.id_map.clone();

    Ok(Self {
      relation_map,
      id_mapping,
      workspace_path: workspace_path.to_path_buf(),
    })
  }

  pub fn build_folder_collab(
    &self,
    uid: i64,
    device_id: &str,
    workspace_name: &str,
  ) -> Result<Folder> {
    let id_mapper = IdMapper {
      id_map: self.id_mapping.clone(),
    };
    FolderCollabRemapper::remap_to_folder_collab(
      &self.relation_map,
      &id_mapper,
      uid,
      device_id,
      workspace_name,
    )
  }

  pub async fn build_database_collabs(&self) -> Result<Vec<Database>> {
    let mut databases = Vec::new();

    for (view_id, collab_metadata) in &self.relation_map.collab_objects {
      if collab_metadata.collab_type == CollabType::Database {
        let json_path = self
          .workspace_path
          .join("collab_jsons")
          .join("databases")
          .join(format!("{}.json", view_id));

        if !json_path.exists() {
          return Err(anyhow!(
            "database json file not found: {}",
            json_path.display()
          ));
        }

        let json_content = fs::read_to_string(&json_path)?;
        let database_json: serde_json::Value = serde_json::from_str(&json_content)?;

        let remapper = DatabaseCollabRemapper::new(database_json, self.id_mapping.clone());
        let database = remapper.build_database().await?;
        databases.push(database);
      }
    }

    Ok(databases)
  }

  pub fn build_document_collabs(&self) -> Result<Vec<Document>> {
    let mut documents = Vec::new();

    for (view_id, collab_metadata) in &self.relation_map.collab_objects {
      if collab_metadata.collab_type == CollabType::Document {
        let json_path = self
          .workspace_path
          .join("collab_jsons")
          .join("documents")
          .join(format!("{}.json", view_id));

        if !json_path.exists() {
          return Err(anyhow!(
            "document json file not found: {}",
            json_path.display()
          ));
        }

        let json_content = fs::read_to_string(&json_path)?;
        let document_json: serde_json::Value = serde_json::from_str(&json_content)?;

        let mapped_view_id = self
          .id_mapping
          .get(view_id)
          .ok_or_else(|| anyhow!("no mapping found for view_id: {}", view_id))?;

        let remapper = DocumentCollabRemapper::new(document_json, self.id_mapping.clone());
        let document = remapper.build_document(mapped_view_id)?;
        documents.push(document);
      }
    }

    Ok(documents)
  }

  pub async fn build_all_collabs(
    &self,
    uid: i64,
    device_id: &str,
    workspace_name: &str,
  ) -> Result<WorkspaceCollabs> {
    let folder = self.build_folder_collab(uid, device_id, workspace_name)?;
    let databases = self.build_database_collabs().await?;
    let documents = self.build_document_collabs()?;

    Ok(WorkspaceCollabs {
      folder,
      databases,
      documents,
    })
  }
}
