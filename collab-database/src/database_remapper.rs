use collab::core::collab::{CollabOptions, DataSource};
use collab::core::origin::CollabOrigin;

use collab::preclude::*;
use std::collections::HashMap;
use std::sync::Arc;

use crate::database::timestamp;
use crate::database::{
  Database, DatabaseBody, DatabaseContext, DatabaseData, default_database_collab,
};
use crate::database_trait::NoPersistenceDatabaseCollabService;
use crate::entity::{CreateDatabaseParams, CreateViewParams, DatabaseView};
use crate::error::DatabaseError;
use crate::rows::{CreateRowParams, Row, RowId};
use crate::views::{OrderObjectPosition, RowOrder};
use collab_entity::CollabType;

pub struct DatabaseCollabRemapper {
  id_mapping: HashMap<String, String>,
}

impl DatabaseCollabRemapper {
  pub fn new(id_mapping: HashMap<String, String>) -> Self {
    Self { id_mapping }
  }

  /// remap the collab database
  ///
  /// 1. replace all the row id
  /// 2. replace all the order id
  /// 3 .replace all the view id
  /// 4. replace all the relation id (not supported yet)
  pub async fn remap_database_collab_state(
    &self,
    database_id: &str,
    user_id: &str,
    db_state: &Vec<u8>,
  ) -> Result<Vec<u8>, DatabaseError> {
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
    db_state: &Vec<u8>,
  ) -> Result<DatabaseData, DatabaseError> {
    let client_id = user_id.parse::<u64>().unwrap_or(0);

    let data_source = DataSource::DocStateV1(db_state.clone());

    let options =
      CollabOptions::new(database_id.to_string(), client_id).with_data_source(data_source);
    let collab = Collab::new_with_options(CollabOrigin::Empty, options)
      .map_err(|e| DatabaseError::Internal(anyhow::Error::new(e)))?;
    let collab_service = Arc::new(NoPersistenceDatabaseCollabService::new(client_id));
    let database_body = DatabaseBody::from_collab(&collab, collab_service.clone(), None)
      .ok_or_else(|| {
        DatabaseError::NoRequiredData("Cannot parse database from collab".to_string())
      })?;
    let database = Database {
      collab,
      body: database_body,
      collab_service,
    };
    let database_data = database.get_database_data(20, false).await;
    Ok(database_data)
  }

  async fn database_data_to_collab_bytes(
    &self,
    database_data: DatabaseData,
    _database_id: &str,
    user_id: &str,
  ) -> Result<Vec<u8>, DatabaseError> {
    let client_id = user_id.parse::<u64>().unwrap_or(0);

    let remapped_database_id = database_data.database_id.clone();
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

    let encoded_collab = crate::util::encoded_collab(&collab, &CollabType::Database)?;

    let bytes = encoded_collab.doc_state.to_vec();

    Ok(bytes)
  }

  pub fn create_database_params_from_remapped_data(
    &self,
    data: DatabaseData,
  ) -> CreateDatabaseParams {
    let timestamp = timestamp();

    let database_id = data.database_id.clone();

    let create_row_params = data
      .rows
      .into_iter()
      .map(|row| CreateRowParams {
        id: row.id,
        database_id: database_id.clone(),
        created_at: timestamp,
        modified_at: timestamp,
        cells: row.cells,
        height: row.height,
        visibility: row.visibility,
        row_position: OrderObjectPosition::End,
      })
      .collect();

    let create_view_params = data
      .views
      .into_iter()
      .map(|view| CreateViewParams {
        database_id: database_id.clone(),
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
      .map(|row| CreateRowParams {
        id: row.id,
        database_id: data.database_id.clone(),
        created_at: timestamp,
        modified_at: timestamp,
        cells: row.cells,
        height: row.height,
        visibility: row.visibility,
        row_position: OrderObjectPosition::End,
      })
      .collect();

    let create_view_params = data
      .views
      .into_iter()
      .map(|view| CreateViewParams {
        database_id: data.database_id.clone(),
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
      database_id: data.database_id.clone(),
      rows: create_row_params,
      fields: data.fields,
      views: create_view_params,
    }
  }

  pub fn remap_database_data(
    &self,
    mut database_data: DatabaseData,
  ) -> Result<DatabaseData, DatabaseError> {
    if let Some(new_database_id) = self.id_mapping.get(&database_data.database_id) {
      database_data.database_id = new_database_id.clone();
    }
    database_data.views = self.remap_database_views(database_data.views);
    database_data.rows = self.remap_database_rows(database_data.rows);

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
      view.id = new_view_id.clone();
    }

    if let Some(new_database_id) = self.id_mapping.get(&view.database_id) {
      view.database_id = new_database_id.clone();
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
    if let Some(new_row_id) = self.id_mapping.get(&row.id.to_string()) {
      row.id = RowId::from(new_row_id.clone());
    }

    if let Some(new_database_id) = self.id_mapping.get(&row.database_id) {
      row.database_id = new_database_id.clone();
    }

    row
  }

  fn remap_row_orders(&self, row_orders: Vec<RowOrder>) -> Vec<RowOrder> {
    row_orders
      .into_iter()
      .map(|mut row_order| {
        if let Some(new_row_id) = self.id_mapping.get(&row_order.id.to_string()) {
          row_order.id = RowId::from(new_row_id.clone());
        }
        row_order
      })
      .collect()
  }
}
