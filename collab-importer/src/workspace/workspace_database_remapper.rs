use anyhow::Result;
use collab::core::collab::CollabOptions;
use collab::core::collab::default_client_id;
use collab::core::origin::CollabOrigin;
use collab::preclude::Collab;
use collab_database::workspace_database::WorkspaceDatabase;
use serde::{Deserialize, Serialize};
use serde_json;
use std::collections::HashMap;

use crate::workspace::id_remapper::JsonIdRemapper;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DatabaseMetaData {
  pub database_id: String,
  pub view_ids: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WorkspaceDatabaseData {
  pub databases: Vec<DatabaseMetaData>,
}

impl WorkspaceDatabaseData {
  pub fn new(databases: Vec<DatabaseMetaData>) -> Self {
    Self { databases }
  }
}

pub struct WorkspaceDatabaseRemapper {
  id_mapping: HashMap<String, String>,
  workspace_database_json: serde_json::Value,
}

impl WorkspaceDatabaseRemapper {
  pub fn new(
    workspace_database_json: serde_json::Value,
    id_mapping: HashMap<String, String>,
  ) -> Self {
    Self {
      id_mapping,
      workspace_database_json,
    }
  }

  pub fn remap_json(&self) -> Result<serde_json::Value> {
    let mut json_value = self.workspace_database_json.clone();
    let remapper = JsonIdRemapper::new(&self.id_mapping);
    remapper.remap_json_value(&mut json_value);
    Ok(json_value)
  }

  pub fn build_workspace_database_data(&self) -> Result<WorkspaceDatabaseData> {
    let remapped_json = self.remap_json()?;
    let workspace_database_data: WorkspaceDatabaseData = serde_json::from_value(remapped_json)?;
    Ok(workspace_database_data)
  }

  pub fn build_workspace_database(&self, database_storage_id: &str) -> Result<WorkspaceDatabase> {
    let workspace_database_data = self.build_workspace_database_data()?;
    let options = CollabOptions::new(database_storage_id.to_string(), default_client_id());
    let collab = Collab::new_with_options(CollabOrigin::Empty, options)?;
    let mut workspace_database = WorkspaceDatabase::create(collab);

    let mut database_map = HashMap::new();
    for database_meta_data in workspace_database_data.databases {
      database_map.insert(
        database_meta_data.database_id.clone(),
        database_meta_data.view_ids.clone(),
      );
    }

    if !database_map.is_empty() {
      workspace_database.batch_add_database(database_map);
    }

    Ok(workspace_database)
  }
}
