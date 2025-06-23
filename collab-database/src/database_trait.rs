use crate::database::{Database, DatabaseBody, DatabaseContext, default_database_collab};

use crate::entity::CreateDatabaseParams;
use crate::error::DatabaseError;
use crate::rows::{DatabaseRow, Row, RowChangeSender, RowId, default_database_row_from_row};
use anyhow::anyhow;
use async_trait::async_trait;
use collab::core::collab::{CollabOptions, DataSource};
use collab::core::collab_plugin::CollabPersistence;
use collab::core::origin::CollabOrigin;
use collab::entity::EncodedCollab;
use collab::error::CollabError;
use collab::lock::RwLock;
use collab::preclude::Collab;
use collab_entity::CollabType;
use dashmap::DashMap;
use futures::future::join_all;
use rayon::prelude::*;
use std::borrow::BorrowMut;
use std::collections::HashMap;
use std::sync::Arc;
use yrs::block::ClientID;

// Database holder tracks initialization status and holds the database reference
pub enum DatabaseDataVariant {
  Params(CreateDatabaseParams),
  EncodedCollab(EncodedCollab),
}

pub enum DatabaseRowDataVariant {
  Row(Row),
  EncodedCollab(EncodedCollab),
}

impl DatabaseRowDataVariant {
  pub fn into_encode_collab(self, client_id: ClientID) -> EncodedCollab {
    match self {
      DatabaseRowDataVariant::Row(row) => default_database_row_from_row(row, client_id),
      DatabaseRowDataVariant::EncodedCollab(encoded_collab) => encoded_collab,
    }
  }
}

pub type EncodeCollabByOid = HashMap<String, EncodedCollab>;
pub type DataSourceByOid = HashMap<String, DataSource>;
pub type CollabRef = Arc<RwLock<dyn BorrowMut<Collab> + Send + Sync + 'static>>;
/// Use this trait to build a [MutexCollab] for a database object including [Database],
/// [DatabaseView], and [DatabaseRow]. When building a [MutexCollab], the caller can add
/// different [CollabPlugin]s to the [MutexCollab] to support different features.
///
#[async_trait]
pub trait DatabaseCollabService: Send + Sync + 'static {
  async fn database_client_id(&self) -> ClientID;

  async fn build_arc_database(
    &self,
    object_id: &str,
    is_new: bool,
    data: Option<DatabaseDataVariant>,
    context: DatabaseContext,
  ) -> Result<Arc<RwLock<Database>>, DatabaseError> {
    let database = self
      .build_database(object_id, is_new, data, context)
      .await?;
    Ok(Arc::new(RwLock::new(database)))
  }

  async fn build_database(
    &self,
    object_id: &str,
    is_new: bool,
    data: Option<DatabaseDataVariant>,
    context: DatabaseContext,
  ) -> Result<Database, DatabaseError>;

  async fn build_workspace_database_collab(
    &self,
    object_id: &str,
    encoded_collab: Option<EncodedCollab>,
  ) -> Result<Collab, DatabaseError>;

  fn persistence(&self) -> Option<Arc<dyn DatabaseCollabPersistenceService>>;
}

#[async_trait]
pub trait DatabaseRowCollabService: Send + Sync + 'static {
  async fn database_row_client_id(&self) -> ClientID;

  async fn create_arc_database_row(
    &self,
    object_id: &str,
    data: DatabaseRowDataVariant,
    sender: Option<RowChangeSender>,
  ) -> Result<Arc<RwLock<DatabaseRow>>, DatabaseError>;

  async fn build_arc_database_row(
    &self,
    object_id: &str,
    data: Option<DatabaseRowDataVariant>,
    sender: Option<RowChangeSender>,
  ) -> Result<Arc<RwLock<DatabaseRow>>, DatabaseError>;

  async fn batch_build_arc_database_row(
    &self,
    row_ids: &[String],
    sender: Option<RowChangeSender>,
    auto_fetch: bool,
  ) -> Result<HashMap<RowId, Arc<RwLock<DatabaseRow>>>, DatabaseError>;
}

#[async_trait]
pub trait DatabaseCollabReader: Send + Sync + 'static {
  async fn reader_client_id(&self) -> ClientID;

  async fn reader_get_collab(
    &self,
    object_id: &str,
    collab_type: CollabType,
  ) -> Result<EncodedCollab, DatabaseError>;

  async fn reader_batch_get_collabs(
    &self,
    object_ids: Vec<String>,
    collab_type: CollabType,
  ) -> Result<EncodeCollabByOid, DatabaseError>;

  fn reader_persistence(&self) -> Option<Arc<dyn DatabaseCollabPersistenceService>> {
    None
  }

  fn database_row_cache(&self) -> Option<Arc<DashMap<RowId, Arc<RwLock<DatabaseRow>>>>>;
}

#[async_trait]
impl<T> DatabaseCollabService for T
where
  T: DatabaseCollabReader + Send + Sync + 'static,
{
  async fn database_client_id(&self) -> ClientID {
    self.reader_client_id().await
  }

  async fn build_database(
    &self,
    object_id: &str,
    _is_new: bool,
    data: Option<DatabaseDataVariant>,
    context: DatabaseContext,
  ) -> Result<Database, DatabaseError> {
    let client_id = self.reader_client_id().await;
    let collab_service = context.database_collab_service.clone();
    let collab_type = CollabType::Database;
    let (body, collab) = match data {
      None => {
        let data = self.reader_get_collab(object_id, collab_type).await?;
        let collab = build_collab(client_id, object_id, collab_type, data).await?;
        DatabaseBody::open(collab, context)?
      },
      Some(data) => match data {
        DatabaseDataVariant::Params(params) => {
          let database_id = params.database_id.clone();
          let (body, collab) =
            default_database_collab(&database_id, client_id, Some(params), context.clone()).await?;
          (body, collab)
        },
        DatabaseDataVariant::EncodedCollab(data) => {
          let collab = build_collab(client_id, object_id, collab_type, data).await?;
          DatabaseBody::open(collab, context)?
        },
      },
    };

    Ok(Database {
      collab,
      body,
      collab_service,
    })
  }

  async fn build_workspace_database_collab(
    &self,
    object_id: &str,
    encoded_collab: Option<EncodedCollab>,
  ) -> Result<Collab, DatabaseError> {
    let collab_type = CollabType::WorkspaceDatabase;
    let client_id = self.reader_client_id().await;
    match encoded_collab {
      Some(encoded_collab) => {
        let collab = build_collab(client_id, object_id, collab_type, encoded_collab).await?;
        Ok(collab)
      },
      None => {
        let data = self
          .reader_get_collab(object_id, CollabType::WorkspaceDatabase)
          .await?;
        let collab = build_collab(client_id, object_id, collab_type, data).await?;
        Ok(collab)
      },
    }
  }

  fn persistence(&self) -> Option<Arc<dyn DatabaseCollabPersistenceService>> {
    self.reader_persistence()
  }
}

#[async_trait]
impl<T> DatabaseRowCollabService for T
where
  T: DatabaseCollabReader + Send + Sync + 'static,
{
  async fn database_row_client_id(&self) -> ClientID {
    self.reader_client_id().await
  }

  async fn create_arc_database_row(
    &self,
    object_id: &str,
    data: DatabaseRowDataVariant,
    sender: Option<RowChangeSender>,
  ) -> Result<Arc<RwLock<DatabaseRow>>, DatabaseError> {
    let client_id = self.reader_client_id().await;
    let collab_type = CollabType::DatabaseRow;
    let data = data.into_encode_collab(client_id);

    if let Some(persistence) = self.reader_persistence() {
      persistence.upsert_collab(object_id, data.clone())?;
    }

    let collab = build_collab(client_id, object_id, collab_type, data).await?;
    let row_id = RowId::from(object_id);
    let database_row = DatabaseRow::open(row_id.clone(), collab, sender)?;
    let arc_row = Arc::new(RwLock::new(database_row));
    if let Some(cache) = self.database_row_cache() {
      cache.insert(row_id, arc_row.clone());
    }
    Ok(arc_row)
  }

  async fn build_arc_database_row(
    &self,
    object_id: &str,
    data: Option<DatabaseRowDataVariant>,
    sender: Option<RowChangeSender>,
  ) -> Result<Arc<RwLock<DatabaseRow>>, DatabaseError> {
    let row_id = RowId::from(object_id);
    if let Some(cache) = self.database_row_cache() {
      if let Some(cached_row) = cache.get(&row_id) {
        return Ok(cached_row.clone());
      }
    }

    let client_id = self.reader_client_id().await;
    let collab_type = CollabType::DatabaseRow;
    let data = match data {
      None => self.reader_get_collab(object_id, collab_type).await?,
      Some(data) => data.into_encode_collab(client_id),
    };
    let collab = build_collab(client_id, object_id, collab_type, data).await?;
    let database_row = DatabaseRow::open(RowId::from(object_id), collab, sender)?;
    let arc_row = Arc::new(RwLock::new(database_row));

    if let Some(cache) = self.database_row_cache() {
      cache.insert(row_id, arc_row.clone());
    }
    Ok(arc_row)
  }

  async fn batch_build_arc_database_row(
    &self,
    row_ids: &[String],
    sender: Option<RowChangeSender>,
    _auto_fetch: bool,
  ) -> Result<HashMap<RowId, Arc<RwLock<DatabaseRow>>>, DatabaseError> {
    let mut result = HashMap::new();
    let mut uncached_row_ids = Vec::new();

    // First, get rows from cache if available
    if let Some(cache) = self.database_row_cache() {
      for row_id_str in row_ids {
        let row_id = RowId::from(row_id_str.as_str());
        if let Some(cached_row) = cache.get(&row_id) {
          result.insert(row_id, cached_row.clone());
        } else {
          uncached_row_ids.push(row_id_str.clone());
        }
      }
    } else {
      // If no cache available, all rows need to be fetched
      uncached_row_ids = row_ids.to_vec();
    }

    // Fetch collabs for the uncached row IDs only
    if !uncached_row_ids.is_empty() {
      let encoded_collab_by_id = self
        .reader_batch_get_collabs(uncached_row_ids, CollabType::DatabaseRow)
        .await?;

      // Prepare concurrent tasks to initialize database rows
      let futures = encoded_collab_by_id
        .into_iter()
        .map(|(row_id, encoded_collab)| async {
          let row_id = RowId::from(row_id);
          let database_row = self
            .build_arc_database_row(
              &row_id,
              Some(DatabaseRowDataVariant::EncodedCollab(encoded_collab)),
              sender.clone(),
            )
            .await?;
          Ok::<_, DatabaseError>((row_id, database_row))
        });

      // Execute the tasks concurrently and collect them into the result HashMap
      let uncached_rows: HashMap<RowId, Arc<RwLock<DatabaseRow>>> = join_all(futures)
        .await
        .into_iter()
        .collect::<Result<HashMap<_, _>, _>>()?;

      // Add the newly fetched rows to the result
      result.extend(uncached_rows);
    }

    Ok(result)
  }
}
async fn build_collab(
  client_id: ClientID,
  object_id: &str,
  _object_type: CollabType,
  encoded_collab: EncodedCollab,
) -> Result<Collab, DatabaseError> {
  let options =
    CollabOptions::new(object_id.to_string(), client_id).with_data_source(encoded_collab.into());
  Ok(Collab::new_with_options(CollabOrigin::Empty, options).unwrap())
}

#[derive(Clone)]
pub struct NoPersistenceDatabaseCollabService {
  pub client_id: ClientID,
  cache: Arc<DashMap<RowId, Arc<RwLock<DatabaseRow>>>>,
}

impl NoPersistenceDatabaseCollabService {
  pub fn new(client_id: ClientID) -> Self {
    Self {
      client_id,
      cache: Arc::new(DashMap::new()),
    }
  }
}

#[async_trait]
impl DatabaseCollabReader for NoPersistenceDatabaseCollabService {
  async fn reader_client_id(&self) -> ClientID {
    self.client_id
  }

  async fn reader_get_collab(
    &self,
    object_id: &str,
    _collab_type: CollabType,
  ) -> Result<EncodedCollab, DatabaseError> {
    Err(DatabaseError::Internal(anyhow!(
      "No persistence service available to get collab for {}",
      object_id
    )))
  }

  async fn reader_batch_get_collabs(
    &self,
    object_ids: Vec<String>,
    collab_type: CollabType,
  ) -> Result<EncodeCollabByOid, DatabaseError> {
    let map: HashMap<String, _> = object_ids
      .into_par_iter()
      .filter_map(|object_id| {
        let persistence = self.persistence();
        let options = CollabOptions::new(object_id.to_string(), self.client_id)
          .with_data_source(CollabPersistenceImpl { persistence }.into());
        let result = Collab::new_with_options(CollabOrigin::Empty, options)
          .map_err(|err| DatabaseError::Internal(err.into()))
          .and_then(|collab| {
            collab
              .encode_collab_v1(|collab| {
                collab_type.validate_require_data(collab)?;
                Ok(())
              })
              .map_err(DatabaseError::Internal)
          });

        match result {
          Ok(encoded_collab) => Some((object_id, encoded_collab)),
          Err(_) => None,
        }
      })
      .collect();

    Ok(map)
  }

  fn database_row_cache(&self) -> Option<Arc<DashMap<RowId, Arc<RwLock<DatabaseRow>>>>> {
    Some(self.cache.clone())
  }
}

pub trait DatabaseCollabPersistenceService: Send + Sync + 'static {
  fn load_collab(&self, collab: &mut Collab);
  fn upsert_collab(
    &self,
    object_id: &str,
    encoded_collab: EncodedCollab,
  ) -> Result<(), DatabaseError>;

  fn get_encoded_collab(&self, object_id: &str, collab_type: CollabType) -> Option<EncodedCollab>;

  fn delete_collab(&self, object_id: &str) -> Result<(), DatabaseError>;

  fn is_collab_exist(&self, object_id: &str) -> bool;
}

pub struct CollabPersistenceImpl {
  pub persistence: Option<Arc<dyn DatabaseCollabPersistenceService>>,
}
impl CollabPersistence for CollabPersistenceImpl {
  fn load_collab_from_disk(&self, collab: &mut Collab) -> Result<(), CollabError> {
    if let Some(persistence) = &self.persistence {
      persistence.load_collab(collab);
    }
    Ok(())
  }
}

impl From<CollabPersistenceImpl> for DataSource {
  fn from(persistence: CollabPersistenceImpl) -> Self {
    DataSource::Disk(Some(Box::new(persistence)))
  }
}
