use crate::database::database::{Database, DatabaseBody, DatabaseContext, default_database_collab};

use crate::core::collab::{CollabOptions, DataSource};
use crate::core::collab_plugin::CollabPersistence;
use crate::core::origin::CollabOrigin;
use crate::database::entity::CreateDatabaseParams;
use crate::database::rows::{DatabaseRow, Row, RowChangeSender, default_database_row_from_row};
use crate::entity::CollabType;
use crate::entity::EncodedCollab;
use crate::entity::uuid_validation::{ObjectId, RowId};
use crate::error::CollabError;
use crate::lock::RwLock;
use crate::preclude::Collab;
use anyhow::anyhow;
use async_trait::async_trait;
use dashmap::DashMap;
use futures::future::join_all;
use rayon::prelude::*;
use std::borrow::BorrowMut;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::instrument;
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

pub type EncodeCollabByOid = HashMap<ObjectId, EncodedCollab>;
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
    object_id: &ObjectId,
    is_new: bool,
    data: Option<DatabaseDataVariant>,
    context: DatabaseContext,
  ) -> Result<Arc<RwLock<Database>>, CollabError> {
    let database = self
      .build_database(object_id, is_new, data, context)
      .await?;
    Ok(Arc::new(RwLock::new(database)))
  }

  async fn build_database(
    &self,
    object_id: &ObjectId,
    is_new: bool,
    data: Option<DatabaseDataVariant>,
    context: DatabaseContext,
  ) -> Result<Database, CollabError>;

  async fn build_workspace_database_collab(
    &self,
    object_id: &ObjectId,
    encoded_collab: Option<EncodedCollab>,
  ) -> Result<Collab, CollabError>;

  fn persistence(&self) -> Option<Arc<dyn DatabaseCollabPersistenceService>>;
}

#[async_trait]
pub trait DatabaseRowCollabService: Send + Sync + 'static {
  async fn database_row_client_id(&self) -> ClientID;

  async fn create_arc_database_row(
    &self,
    row_id: &RowId,
    data: DatabaseRowDataVariant,
    sender: Option<RowChangeSender>,
  ) -> Result<Arc<RwLock<DatabaseRow>>, CollabError>;

  async fn build_arc_database_row(
    &self,
    row_id: &RowId,
    data: Option<DatabaseRowDataVariant>,
    sender: Option<RowChangeSender>,
  ) -> Result<Arc<RwLock<DatabaseRow>>, CollabError>;

  async fn batch_build_arc_database_row(
    &self,
    row_ids: &[RowId],
    sender: Option<RowChangeSender>,
    auto_fetch: bool,
  ) -> Result<HashMap<RowId, Arc<RwLock<DatabaseRow>>>, CollabError>;

  fn database_row_cache(&self) -> Option<Arc<DashMap<RowId, Arc<RwLock<DatabaseRow>>>>> {
    None
  }
}

#[async_trait]
pub trait DatabaseCollabReader: Send + Sync + 'static {
  async fn reader_client_id(&self) -> ClientID;

  async fn reader_get_collab(
    &self,
    object_id: &ObjectId,
    collab_type: CollabType,
  ) -> Result<EncodedCollab, CollabError>;

  async fn reader_batch_get_collabs(
    &self,
    object_ids: Vec<ObjectId>,
    collab_type: CollabType,
  ) -> Result<EncodeCollabByOid, CollabError>;

  fn reader_persistence(&self) -> Option<Arc<dyn DatabaseCollabPersistenceService>> {
    None
  }

  fn reader_row_cache(&self) -> Option<Arc<DashMap<RowId, Arc<RwLock<DatabaseRow>>>>> {
    None
  }
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
    object_id: &ObjectId,
    _is_new: bool,
    data: Option<DatabaseDataVariant>,
    context: DatabaseContext,
  ) -> Result<Database, CollabError> {
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
          let database_id = params.database_id;
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
    object_id: &ObjectId,
    encoded_collab: Option<EncodedCollab>,
  ) -> Result<Collab, CollabError> {
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
    row_id: &RowId,
    data: DatabaseRowDataVariant,
    sender: Option<RowChangeSender>,
  ) -> Result<Arc<RwLock<DatabaseRow>>, CollabError> {
    let client_id = self.reader_client_id().await;
    let collab_type = CollabType::DatabaseRow;
    let data = data.into_encode_collab(client_id);
    if let Some(persistence) = self.reader_persistence() {
      persistence.upsert_collab(row_id, data.clone())?;
    }

    let collab = build_collab(client_id, row_id, collab_type, data).await?;
    let database_row = DatabaseRow::open(*row_id, collab, sender)?;
    let arc_row = Arc::new(RwLock::new(database_row));
    if let Some(cache) = self.database_row_cache() {
      cache.insert(*row_id, arc_row.clone());
    }
    Ok(arc_row)
  }

  async fn build_arc_database_row(
    &self,
    row_id: &RowId,
    data: Option<DatabaseRowDataVariant>,
    sender: Option<RowChangeSender>,
  ) -> Result<Arc<RwLock<DatabaseRow>>, CollabError> {
    if let Some(cache) = self.database_row_cache() {
      if let Some(cached_row) = cache.get(row_id) {
        return Ok(cached_row.clone());
      }
    }

    let client_id = self.reader_client_id().await;
    let collab_type = CollabType::DatabaseRow;
    let data = match data {
      None => self.reader_get_collab(row_id, collab_type).await?,
      Some(data) => data.into_encode_collab(client_id),
    };
    let collab = build_collab(client_id, row_id, collab_type, data).await?;
    let database_row = DatabaseRow::open(*row_id, collab, sender)?;
    let arc_row = Arc::new(RwLock::new(database_row));

    if let Some(cache) = self.database_row_cache() {
      cache.insert(*row_id, arc_row.clone());
    }
    Ok(arc_row)
  }

  #[instrument(level = "debug", skip_all)]
  async fn batch_build_arc_database_row(
    &self,
    row_ids: &[RowId],
    sender: Option<RowChangeSender>,
    _auto_fetch: bool,
  ) -> Result<HashMap<RowId, Arc<RwLock<DatabaseRow>>>, CollabError> {
    let mut result = HashMap::new();
    let mut uncached_row_ids = Vec::new();

    // First, get rows from cache if available
    if let Some(cache) = self.database_row_cache() {
      for row_id in row_ids {
        if let Some(cached_row) = cache.get(row_id) {
          result.insert(*row_id, cached_row.clone());
        } else {
          uncached_row_ids.push(*row_id);
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
      let sender_clone = sender.clone();
      let futures = encoded_collab_by_id
        .into_iter()
        .map(|(row_id, encoded_collab)| {
          let sender = sender_clone.clone();
          let this = self;
          async move {
            let database_row = this
              .build_arc_database_row(
                &row_id,
                Some(DatabaseRowDataVariant::EncodedCollab(encoded_collab)),
                sender,
              )
              .await?;
            Ok::<_, CollabError>((row_id, database_row))
          }
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

  fn database_row_cache(&self) -> Option<Arc<DashMap<RowId, Arc<RwLock<DatabaseRow>>>>> {
    DatabaseCollabReader::reader_row_cache(self)
  }
}

/// Adapter to provide a dedicated row cache for a DatabaseCollabReader-backed service.
#[derive(Clone)]
pub struct DatabaseRowCollabServiceAdapter<R> {
  reader: Arc<R>,
  row_cache: Option<Arc<DashMap<RowId, Arc<RwLock<DatabaseRow>>>>>,
}

impl<R> DatabaseRowCollabServiceAdapter<R> {
  pub fn new(reader: Arc<R>) -> Self {
    Self {
      reader,
      row_cache: None,
    }
  }

  pub fn new_with_cache(
    reader: Arc<R>,
    row_cache: Arc<DashMap<RowId, Arc<RwLock<DatabaseRow>>>>,
  ) -> Self {
    Self {
      reader,
      row_cache: Some(row_cache),
    }
  }
}

#[async_trait]
impl<R> DatabaseRowCollabService for DatabaseRowCollabServiceAdapter<R>
where
  R: DatabaseCollabReader + Send + Sync + 'static,
{
  async fn database_row_client_id(&self) -> ClientID {
    self.reader.reader_client_id().await
  }

  fn database_row_cache(&self) -> Option<Arc<DashMap<RowId, Arc<RwLock<DatabaseRow>>>>> {
    self.row_cache.clone()
  }

  async fn create_arc_database_row(
    &self,
    row_id: &RowId,
    data: DatabaseRowDataVariant,
    sender: Option<RowChangeSender>,
  ) -> Result<Arc<RwLock<DatabaseRow>>, CollabError> {
    let client_id = self.reader.reader_client_id().await;
    let collab_type = CollabType::DatabaseRow;
    let data = data.into_encode_collab(client_id);
    if let Some(persistence) = self.reader.reader_persistence() {
      persistence.upsert_collab(row_id, data.clone())?;
    }

    let collab = build_collab(client_id, row_id, collab_type, data).await?;
    let database_row = DatabaseRow::open(*row_id, collab, sender)?;
    let arc_row = Arc::new(RwLock::new(database_row));
    if let Some(cache) = self.database_row_cache() {
      cache.insert(*row_id, arc_row.clone());
    }
    Ok(arc_row)
  }

  async fn build_arc_database_row(
    &self,
    row_id: &RowId,
    data: Option<DatabaseRowDataVariant>,
    sender: Option<RowChangeSender>,
  ) -> Result<Arc<RwLock<DatabaseRow>>, CollabError> {
    if let Some(cache) = self.database_row_cache() {
      if let Some(cached_row) = cache.get(row_id) {
        return Ok(cached_row.clone());
      }
    }

    let client_id = self.reader.reader_client_id().await;
    let collab_type = CollabType::DatabaseRow;
    let data = match data {
      None => self.reader.reader_get_collab(row_id, collab_type).await?,
      Some(data) => data.into_encode_collab(client_id),
    };
    let collab = build_collab(client_id, row_id, collab_type, data).await?;
    let database_row = DatabaseRow::open(*row_id, collab, sender)?;
    let arc_row = Arc::new(RwLock::new(database_row));

    if let Some(cache) = self.database_row_cache() {
      cache.insert(*row_id, arc_row.clone());
    }
    Ok(arc_row)
  }

  #[tracing::instrument(level = "debug", skip_all)]
  async fn batch_build_arc_database_row(
    &self,
    row_ids: &[RowId],
    sender: Option<RowChangeSender>,
    _auto_fetch: bool,
  ) -> Result<HashMap<RowId, Arc<RwLock<DatabaseRow>>>, CollabError> {
    let mut result = HashMap::new();
    let mut uncached_row_ids = Vec::new();

    if let Some(cache) = self.database_row_cache() {
      for row_id in row_ids {
        if let Some(cached_row) = cache.get(row_id) {
          result.insert(*row_id, cached_row.clone());
        } else {
          uncached_row_ids.push(*row_id);
        }
      }
    } else {
      uncached_row_ids = row_ids.to_vec();
    }

    if !uncached_row_ids.is_empty() {
      let encoded_collab_by_id = self
        .reader
        .reader_batch_get_collabs(uncached_row_ids, CollabType::DatabaseRow)
        .await?;

      let sender_clone = sender.clone();
      let futures = encoded_collab_by_id
        .into_iter()
        .map(|(row_id, encoded_collab)| {
          let sender = sender_clone.clone();
          async move {
            let database_row = self
              .build_arc_database_row(
                &row_id,
                Some(DatabaseRowDataVariant::EncodedCollab(encoded_collab)),
                sender,
              )
              .await?;
            Ok::<_, CollabError>((row_id, database_row))
          }
        });

      let uncached_rows: HashMap<RowId, Arc<RwLock<DatabaseRow>>> = join_all(futures)
        .await
        .into_iter()
        .collect::<Result<HashMap<_, _>, _>>()?;

      result.extend(uncached_rows);
    }

    Ok(result)
  }
}
async fn build_collab(
  client_id: ClientID,
  object_id: &ObjectId,
  _object_type: CollabType,
  encoded_collab: EncodedCollab,
) -> Result<Collab, CollabError> {
  let options = CollabOptions::new(*object_id, client_id).with_data_source(encoded_collab.into());
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

  pub fn new_with_cache(
    client_id: ClientID,
    cache: Arc<DashMap<RowId, Arc<RwLock<DatabaseRow>>>>,
  ) -> Self {
    Self { client_id, cache }
  }
}

#[async_trait]
impl DatabaseCollabReader for NoPersistenceDatabaseCollabService {
  async fn reader_client_id(&self) -> ClientID {
    self.client_id
  }

  async fn reader_get_collab(
    &self,
    object_id: &ObjectId,
    _collab_type: CollabType,
  ) -> Result<EncodedCollab, CollabError> {
    Err(CollabError::Internal(anyhow!(
      "No persistence service available to get collab for {}",
      object_id
    )))
  }

  async fn reader_batch_get_collabs(
    &self,
    object_ids: Vec<ObjectId>,
    collab_type: CollabType,
  ) -> Result<EncodeCollabByOid, CollabError> {
    let map: HashMap<ObjectId, _> = object_ids
      .into_par_iter()
      .filter_map(|object_id| {
        let persistence = self.persistence();
        let options = CollabOptions::new(object_id, self.client_id)
          .with_data_source(CollabPersistenceImpl { persistence }.into());
        let result = Collab::new_with_options(CollabOrigin::Empty, options)
          .map_err(|err| CollabError::Internal(err.into()))
          .and_then(|collab| {
            collab
              .encode_collab_v1(|collab| {
                collab_type.validate_require_data(collab)?;
                Ok(())
              })
              .map_err(CollabError::Internal)
          });

        match result {
          Ok(encoded_collab) => Some((object_id, encoded_collab)),
          Err(_) => None,
        }
      })
      .collect();

    Ok(map)
  }

  fn reader_row_cache(&self) -> Option<Arc<DashMap<RowId, Arc<RwLock<DatabaseRow>>>>> {
    Some(self.cache.clone())
  }
}

pub trait DatabaseCollabPersistenceService: Send + Sync + 'static {
  fn load_collab(&self, collab: &mut Collab);
  fn upsert_collab(
    &self,
    object_id: &ObjectId,
    encoded_collab: EncodedCollab,
  ) -> Result<(), CollabError>;

  fn get_encoded_collab(
    &self,
    object_id: &ObjectId,
    collab_type: CollabType,
  ) -> Option<EncodedCollab>;

  fn delete_collab(&self, object_id: &ObjectId) -> Result<(), CollabError>;

  fn is_collab_exist(&self, object_id: &ObjectId) -> bool;
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
