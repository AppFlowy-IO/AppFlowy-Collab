use crate::core::collab::default_client_id;
use crate::database::database::{Database, DatabaseContext, DatabaseData};
use crate::database::database_remapper::DatabaseCollabRemapper as DatabaseRemapper;
use crate::database::database_trait::NoPersistenceDatabaseCollabService;
use crate::database::entity::CreateDatabaseParams;
use anyhow::Result;
use serde_json;
use std::collections::HashMap;
use std::sync::Arc;

use crate::importer::workspace::id_remapper::JsonIdRemapper;

pub struct DatabaseCollabRemapper {
  id_mapping: HashMap<String, String>,
  database_json: serde_json::Value,
}

impl DatabaseCollabRemapper {
  pub fn new(database_json: serde_json::Value, id_mapping: HashMap<String, String>) -> Self {
    Self {
      id_mapping,
      database_json,
    }
  }

  pub fn remap_json(&self) -> Result<serde_json::Value> {
    let mut json_value = self.database_json.clone();
    let remapper = JsonIdRemapper::new(&self.id_mapping);
    remapper.remap_json_value(&mut json_value);
    Ok(json_value)
  }

  pub fn build_database_data(&self) -> Result<DatabaseData> {
    let remapped_json = self.remap_json()?;
    let database_data: DatabaseData = serde_json::from_value(remapped_json)?;
    Ok(database_data)
  }

  pub fn build_create_database_params(&self) -> Result<CreateDatabaseParams> {
    let database_data = self.build_database_data()?;
    let create_params = DatabaseRemapper::create_database_params_with_mapped_ids(database_data);
    Ok(create_params)
  }

  pub async fn build_database(&self) -> Result<Database> {
    let database_data = self.build_database_data()?;
    let collab_service = Arc::new(NoPersistenceDatabaseCollabService::new(default_client_id()));
    let context = DatabaseContext {
      database_collab_service: collab_service.clone(),
      notifier: Default::default(),
      database_row_collab_service: collab_service,
    };

    let create_params = DatabaseRemapper::create_database_params_with_mapped_ids(database_data);
    let database = Database::create_with_view(create_params, context).await?;
    Ok(database)
  }
}
