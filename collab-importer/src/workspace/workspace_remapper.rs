use anyhow::{Result, anyhow};
use collab_database::database::Database;
use collab_database::workspace_database::WorkspaceDatabase;
use collab_document::document::Document;
use collab_entity::CollabType;
use collab_folder::Folder;
use serde_json;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::workspace::database_collab_remapper::DatabaseCollabRemapper;
use crate::workspace::document_collab_remapper::DocumentCollabRemapper;
use crate::workspace::entities::{DependencyType, WorkspaceRelationMap};
use crate::workspace::folder_collab_remapper::FolderCollabRemapper;
use crate::workspace::id_mapper::IdMapper;
use crate::workspace::relation_map_parser::RelationMapParser;
use crate::workspace::workspace_database_remapper::WorkspaceDatabaseRemapper;

pub struct WorkspaceRemapper {
  #[allow(dead_code)]
  custom_workspace_id: Option<String>,
  relation_map: WorkspaceRelationMap,
  id_mapping: HashMap<String, String>,
  workspace_path: PathBuf,
}

pub struct WorkspaceCollabs {
  pub folder: Folder,
  pub databases: Vec<Database>,
  pub documents: Vec<Document>,
  pub row_documents: Vec<Document>,
  pub workspace_database: Option<WorkspaceDatabase>,
}

impl WorkspaceRemapper {
  pub async fn new(workspace_path: &Path, custom_workspace_id: Option<String>) -> Result<Self> {
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

    let mut id_mapper = IdMapper::new(&relation_map);
    if let Some(ref custom_workspace_id) = custom_workspace_id {
      id_mapper.id_map.insert(
        relation_map.workspace_id.clone(),
        custom_workspace_id.clone(),
      );
    }
    let id_mapping = id_mapper.id_map.clone();

    Ok(Self {
      custom_workspace_id,
      relation_map,
      id_mapping,
      workspace_path: workspace_path.to_path_buf(),
    })
  }

  pub fn build_folder_collab(&self, uid: i64, workspace_name: &str) -> Result<Folder> {
    let id_mapper = IdMapper {
      id_map: self.id_mapping.clone(),
    };
    FolderCollabRemapper::remap_to_folder_collab(
      &self.relation_map,
      &id_mapper,
      uid,
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
          return Err(anyhow!("database json file not found: {:?}", json_path));
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

    let row_document_dependencies = self
      .relation_map
      .dependencies
      .iter()
      .filter(|d| d.dependency_type == DependencyType::DatabaseRowDocument)
      .map(|d| d.target_view_id.clone())
      .collect::<Vec<_>>();

    for (view_id, collab_metadata) in &self.relation_map.collab_objects {
      if row_document_dependencies.contains(view_id) {
        continue;
      }

      if collab_metadata.collab_type == CollabType::Document {
        let json_path = self
          .workspace_path
          .join("collab_jsons")
          .join("documents")
          .join(format!("{}.json", view_id));

        if !json_path.exists() {
          return Err(anyhow!("document json file not found: {:?}", json_path));
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

  pub fn build_workspace_database(
    &self,
    database_storage_id: &str,
  ) -> Result<Option<WorkspaceDatabase>> {
    if self.relation_map.workspace_database_meta.is_none() {
      return Ok(None);
    }
    let workspace_database_json = serde_json::json!({
      "databases": self.relation_map.workspace_database_meta
    });
    let remapper = WorkspaceDatabaseRemapper::new(workspace_database_json, self.id_mapping.clone());
    let workspace_database = remapper.build_workspace_database(database_storage_id)?;
    Ok(Some(workspace_database))
  }

  pub fn build_row_document_collabs(&self) -> Result<Vec<Document>> {
    let mut row_documents = Vec::new();

    for (database_id, collab_metadata) in &self.relation_map.collab_objects {
      if collab_metadata.collab_type == CollabType::Database {
        let row_documents_path = self
          .workspace_path
          .join("collab_jsons")
          .join("databases")
          .join(database_id)
          .join("row_documents");

        if row_documents_path.exists() && row_documents_path.is_dir() {
          for entry in fs::read_dir(&row_documents_path)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("json") {
              let row_document_id = path
                .file_stem()
                .and_then(|s| s.to_str())
                .ok_or_else(|| anyhow!("invalid row document filename"))?;

              let json_content = fs::read_to_string(&path)?;
              let document_json: serde_json::Value = serde_json::from_str(&json_content)?;

              let mapped_row_id = self
                .id_mapping
                .get(row_document_id)
                .unwrap_or(&row_document_id.to_string())
                .clone();

              let remapper = DocumentCollabRemapper::new(document_json, self.id_mapping.clone());
              let document = remapper.build_document(&mapped_row_id)?;
              row_documents.push(document);
            }
          }
        }
      }
    }

    Ok(row_documents)
  }

  pub async fn build_all_collabs(
    &self,
    uid: i64,
    workspace_name: &str,
    database_storage_id: &str,
  ) -> Result<WorkspaceCollabs> {
    let folder = self.build_folder_collab(uid, workspace_name)?;
    let databases = self.build_database_collabs().await?;
    let documents = self.build_document_collabs()?;
    let row_documents = self.build_row_document_collabs()?;
    let workspace_database = self.build_workspace_database(database_storage_id)?;

    Ok(WorkspaceCollabs {
      folder,
      databases,
      documents,
      row_documents,
      workspace_database,
    })
  }
}
