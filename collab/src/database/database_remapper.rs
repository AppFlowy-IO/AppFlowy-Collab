use crate::core::collab::{CollabOptions, DataSource};
use crate::core::origin::CollabOrigin;

use crate::preclude::*;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

use crate::database::database::timestamp;
use crate::database::database::{
  Database, DatabaseBody, DatabaseContext, DatabaseData, default_database_collab,
};
use crate::database::database_trait::NoPersistenceDatabaseCollabService;
use crate::database::entity::{CreateDatabaseParams, CreateViewParams, DatabaseView};
use crate::database::rows::{CreateRowParams, Row, RowMeta};
use crate::database::util::encoded_collab;
use crate::database::views::{OrderObjectPosition, RowOrder};
use crate::entity::CollabType;
use crate::entity::uuid_validation::RowId;
use crate::error::CollabError;

pub struct DatabaseCollabRemapper {
  id_mapping: HashMap<Uuid, Uuid>,
}

impl DatabaseCollabRemapper {
  pub fn new(id_mapping: HashMap<String, String>) -> Self {
    let uuid_mapping = id_mapping
      .into_iter()
      .filter_map(|(k, v)| {
        let key_uuid = Uuid::parse_str(&k).ok()?;
        let value_uuid = Uuid::parse_str(&v).ok()?;
        Some((key_uuid, value_uuid))
      })
      .collect();
    Self {
      id_mapping: uuid_mapping,
    }
  }

  /// remap the collab database
  ///
  /// 1. replace all the row id
  /// 2. replace all the order id
  /// 3. replace all the view id
  /// 4. replace all the relation id (not supported yet)
  pub async fn remap_database_collab_state(
    &self,
    database_id: &str,
    user_id: &str,
    db_state: &[u8],
  ) -> Result<Vec<u8>, CollabError> {
    let database_data = self
      .collab_bytes_to_database_data(database_id, user_id, db_state)
      .await?;
    let remapped_data = self.remap_database_data(database_data)?;
    let remapped_bytes = self
      .database_data_to_collab_bytes(remapped_data, database_id, user_id)
      .await?;

    Ok(remapped_bytes)
  }

  pub async fn collab_bytes_to_database_data(
    &self,
    database_id: &str,
    user_id: &str,
    db_state: &[u8],
  ) -> Result<DatabaseData, CollabError> {
    let client_id = user_id.parse::<u64>().unwrap_or(0);
    let data_source = DataSource::DocStateV1(db_state.to_owned());

    let database_uuid =
      Uuid::parse_str(database_id).map_err(|err| CollabError::Internal(err.into()))?;
    let options = CollabOptions::new(database_uuid, client_id).with_data_source(data_source);
    let collab = Collab::new_with_options(CollabOrigin::Empty, options)
      .map_err(|e| CollabError::Internal(anyhow::Error::new(e)))?;
    let collab_service = Arc::new(NoPersistenceDatabaseCollabService::new(client_id));
    let database_body = DatabaseBody::from_collab(&collab, collab_service.clone(), None)
      .ok_or_else(|| {
        CollabError::NoRequiredData("Cannot parse database from collab".to_string())
      })?;
    let database = Database {
      collab,
      body: database_body,
      collab_service,
    };
    let database_data = database.get_database_data(20, false).await?;
    Ok(database_data)
  }

  async fn database_data_to_collab_bytes(
    &self,
    database_data: DatabaseData,
    _database_id: &str,
    user_id: &str,
  ) -> Result<Vec<u8>, CollabError> {
    let client_id = user_id.parse::<u64>().unwrap_or(0);

    let remapped_database_id = database_data.database_id;
    let create_params = self.create_database_params_from_remapped_data(database_data);

    let context = DatabaseContext::new(
      Arc::new(NoPersistenceDatabaseCollabService::new(client_id)),
      Arc::new(NoPersistenceDatabaseCollabService::new(client_id)),
    );

    let (_database_body, collab) = default_database_collab(
      &remapped_database_id,
      client_id,
      Some(create_params),
      context,
    )
    .await?;

    let encoded_collab = encoded_collab(&collab, &CollabType::Database)?;

    let bytes = encoded_collab.doc_state.to_vec();

    Ok(bytes)
  }

  pub fn create_database_params_from_remapped_data(
    &self,
    data: DatabaseData,
  ) -> CreateDatabaseParams {
    let timestamp = timestamp();

    let database_id = data.database_id;

    let create_row_params = data
      .rows
      .into_iter()
      .map(|row| {
        let row_meta = data.row_metas.get(&row.id).cloned();

        CreateRowParams {
          id: row.id,
          database_id,
          created_at: timestamp,
          modified_at: timestamp,
          cells: row.cells,
          height: row.height,
          visibility: row.visibility,
          row_position: OrderObjectPosition::End,
          row_meta,
        }
      })
      .collect();

    let create_view_params = data
      .views
      .into_iter()
      .map(|view| CreateViewParams {
        database_id,
        view_id: view.id,
        name: view.name,
        layout: view.layout,
        layout_settings: view.layout_settings,
        filters: view.filters,
        group_settings: view.group_settings,
        sorts: view.sorts,
        field_settings: view.field_settings,
        embedded: view.embedded,
        created_at: timestamp,
        modified_at: timestamp,
        ..Default::default()
      })
      .collect();

    CreateDatabaseParams {
      database_id,
      rows: create_row_params,
      fields: data.fields,
      views: create_view_params,
    }
  }

  pub fn create_database_params_with_mapped_ids(data: DatabaseData) -> CreateDatabaseParams {
    let timestamp = timestamp();

    let create_row_params = data
      .rows
      .into_iter()
      .map(|row| {
        let row_meta = data.row_metas.get(&row.id).cloned();
        CreateRowParams {
          id: row.id,
          database_id: data.database_id,
          created_at: timestamp,
          modified_at: timestamp,
          cells: row.cells,
          height: row.height,
          visibility: row.visibility,
          row_position: OrderObjectPosition::End,
          row_meta,
        }
      })
      .collect();

    let create_view_params = data
      .views
      .into_iter()
      .map(|view| CreateViewParams {
        database_id: data.database_id,
        view_id: view.id,
        name: view.name,
        layout: view.layout,
        layout_settings: view.layout_settings,
        filters: view.filters,
        group_settings: view.group_settings,
        sorts: view.sorts,
        field_settings: view.field_settings,
        created_at: timestamp,
        modified_at: timestamp,
        ..Default::default()
      })
      .collect();

    CreateDatabaseParams {
      database_id: data.database_id,
      rows: create_row_params,
      fields: data.fields,
      views: create_view_params,
    }
  }

  pub fn remap_database_data(
    &self,
    mut database_data: DatabaseData,
  ) -> Result<DatabaseData, CollabError> {
    if let Some(new_database_id) = self.id_mapping.get(&database_data.database_id) {
      database_data.database_id = *new_database_id;
    }
    database_data.views = self.remap_database_views(database_data.views);
    database_data.rows = self.remap_database_rows(database_data.rows);
    database_data.row_metas = self.remap_row_metas(database_data.row_metas);

    Ok(database_data)
  }

  fn remap_database_views(&self, views: Vec<DatabaseView>) -> Vec<DatabaseView> {
    views
      .into_iter()
      .map(|view| self.remap_database_view(view))
      .collect()
  }

  pub fn remap_database_view(&self, mut view: DatabaseView) -> DatabaseView {
    if let Some(new_view_id) = self.id_mapping.get(&view.id) {
      view.id = *new_view_id;
    }

    if let Some(new_database_id) = self.id_mapping.get(&view.database_id) {
      view.database_id = *new_database_id;
    }

    view.row_orders = self.remap_row_orders(view.row_orders);
    view
  }

  fn remap_database_rows(&self, rows: Vec<Row>) -> Vec<Row> {
    rows
      .into_iter()
      .map(|row| self.remap_row_data(row))
      .collect()
  }

  fn remap_row_data(&self, mut row: Row) -> Row {
    if let Some(new_row_id) = self.id_mapping.get(&row.id) {
      row.id = *new_row_id;
    }

    if let Some(new_database_id) = self.id_mapping.get(&row.database_id) {
      row.database_id = *new_database_id;
    }

    row
  }

  fn remap_row_orders(&self, row_orders: Vec<RowOrder>) -> Vec<RowOrder> {
    row_orders
      .into_iter()
      .map(|mut row_order| {
        if let Some(new_row_id) = self.id_mapping.get(&row_order.id) {
          row_order.id = *new_row_id;
        }
        row_order
      })
      .collect()
  }

  fn remap_row_metas(&self, row_metas: HashMap<RowId, RowMeta>) -> HashMap<RowId, RowMeta> {
    row_metas
      .into_iter()
      .map(|(row_id, row_meta)| {
        if let Some(new_uuid) = self.id_mapping.get(&row_id) {
          (*new_uuid, row_meta)
        } else {
          (row_id, row_meta)
        }
      })
      .collect()
  }
}
