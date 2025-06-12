use crate::database::{
  Database, DatabaseBody, DatabaseContext, DatabaseData, default_database_collab,
};

use crate::error::DatabaseError;
use crate::workspace_database::body::{DatabaseMeta, WorkspaceDatabase};
use async_trait::async_trait;
use collab::core::collab::{CollabOptions, DataSource};
use collab::preclude::Collab;
use collab_entity::CollabType;

use collab::entity::EncodedCollab;

use crate::entity::{CreateDatabaseParams, CreateViewParams, CreateViewParamsValidator};

use crate::rows::{DatabaseRow, Row, RowChangeSender, RowId, default_database_row_from_row};
use anyhow::anyhow;
use collab::core::collab_plugin::CollabPersistence;
use collab::core::origin::CollabOrigin;
use collab::error::CollabError;
use collab::lock::RwLock;
use dashmap::DashMap;
use rayon::prelude::*;
use std::borrow::{Borrow, BorrowMut};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{error, trace};
use yrs::block::ClientID;

// Database holder tracks initialization status and holds the database reference
struct DatabaseHolder {
  database: Mutex<Option<Arc<RwLock<Database>>>>,
}

impl DatabaseHolder {
  fn new() -> Self {
    Self {
      database: Mutex::new(None),
    }
  }
}

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
  async fn client_id(&self) -> ClientID;

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

  async fn build_arc_database_row(
    &self,
    object_id: &str,
    is_new: bool,
    data: Option<DatabaseRowDataVariant>,
    sender: Option<RowChangeSender>,
    collab_service: Arc<dyn DatabaseCollabService>,
  ) -> Result<Arc<RwLock<DatabaseRow>>, DatabaseError>;

  async fn build_workspace_database_collab(
    &self,
    object_id: &str,
    encoded_collab: Option<EncodedCollab>,
  ) -> Result<Collab, DatabaseError>;

  async fn get_collabs(
    &self,
    object_ids: Vec<String>,
    collab_type: CollabType,
  ) -> Result<EncodeCollabByOid, DatabaseError>;

  fn persistence(&self) -> Option<Arc<dyn DatabaseCollabPersistenceService>>;
}

#[async_trait]
pub trait DatabaseCollabReader: Send + Sync + 'static {
  async fn client_id(&self) -> ClientID;

  async fn get_collab(
    &self,
    object_id: &str,
    collab_type: CollabType,
  ) -> Result<EncodedCollab, DatabaseError>;

  async fn batch_get_collabs(
    &self,
    object_ids: Vec<String>,
    collab_type: CollabType,
  ) -> Result<EncodeCollabByOid, DatabaseError>;
}

#[async_trait]
impl<T> DatabaseCollabService for T
where
  T: DatabaseCollabReader + Send + Sync + 'static,
{
  async fn client_id(&self) -> ClientID {
    self.client_id().await
  }

  async fn build_database(
    &self,
    object_id: &str,
    _is_new: bool,
    data: Option<DatabaseDataVariant>,
    context: DatabaseContext,
  ) -> Result<Database, DatabaseError> {
    let client_id = self.client_id().await;
    let collab_service = context.collab_service.clone();
    let collab_type = CollabType::Database;
    let (body, collab) = match data {
      None => {
        let data = self.get_collab(object_id, collab_type).await?;
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

  async fn build_arc_database_row(
    &self,
    object_id: &str,
    _is_new: bool,
    data: Option<DatabaseRowDataVariant>,
    sender: Option<RowChangeSender>,
    collab_service: Arc<dyn DatabaseCollabService>,
  ) -> Result<Arc<RwLock<DatabaseRow>>, DatabaseError> {
    let client_id = self.client_id().await;
    let collab_type = CollabType::DatabaseRow;
    let data = match data {
      None => self.get_collab(object_id, collab_type).await?,
      Some(data) => data.into_encode_collab(client_id),
    };

    let collab = build_collab(client_id, object_id, collab_type, data).await?;
    let database_row = DatabaseRow::open(RowId::from(object_id), collab, sender, collab_service)?;
    Ok(Arc::new(RwLock::new(database_row)))
  }

  async fn build_workspace_database_collab(
    &self,
    object_id: &str,
    encoded_collab: Option<EncodedCollab>,
  ) -> Result<Collab, DatabaseError> {
    let collab_type = CollabType::WorkspaceDatabase;
    let client_id = self.client_id().await;
    match encoded_collab {
      Some(encoded_collab) => {
        let collab = build_collab(client_id, object_id, collab_type, encoded_collab).await?;
        Ok(collab)
      },
      None => {
        let data = self
          .get_collab(object_id, CollabType::WorkspaceDatabase)
          .await?;
        build_collab(client_id, object_id, collab_type, data).await
      },
    }
  }

  async fn get_collabs(
    &self,
    object_ids: Vec<String>,
    collab_type: CollabType,
  ) -> Result<EncodeCollabByOid, DatabaseError> {
    self.batch_get_collabs(object_ids, collab_type).await
  }

  fn persistence(&self) -> Option<Arc<dyn DatabaseCollabPersistenceService>> {
    None
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

#[derive(Debug, Clone)]
pub struct NoPersistenceDatabaseCollabService {
  pub client_id: ClientID,
}

#[async_trait]
impl DatabaseCollabReader for NoPersistenceDatabaseCollabService {
  async fn client_id(&self) -> ClientID {
    self.client_id
  }

  async fn get_collab(
    &self,
    object_id: &str,
    _collab_type: CollabType,
  ) -> Result<EncodedCollab, DatabaseError> {
    Err(DatabaseError::Internal(anyhow!(
      "No persistence service available to get collab for {}",
      object_id
    )))
  }

  async fn batch_get_collabs(
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
}

pub trait DatabaseCollabPersistenceService: Send + Sync + 'static {
  fn load_collab(&self, collab: &mut Collab);

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

/// A [WorkspaceDatabaseManager] indexes the databases within a workspace.
/// Within a workspace, the view ID is used to identify each database. Therefore, you can use the view_id to retrieve
/// the actual database ID from [WorkspaceDatabaseManager]. Additionally, [WorkspaceDatabaseManager] allows you to obtain a database
/// using its database ID.
///
/// Relation between database ID and view ID:
/// One database ID can have multiple view IDs.
///
pub struct WorkspaceDatabaseManager {
  body: WorkspaceDatabase,
  collab_service: Arc<dyn DatabaseCollabService>,
  /// In memory database handlers with their initialization state.
  /// The key is the database id. The handler will be added when the database is opened or created.
  /// and the handler will be removed when the database is deleted or closed.
  database_holders: DashMap<String, Arc<DatabaseHolder>>,
}

impl WorkspaceDatabaseManager {
  pub fn open(
    _object_id: &str,
    collab: Collab,
    collab_service: impl DatabaseCollabService,
  ) -> Result<Self, DatabaseError> {
    let collab_service = Arc::new(collab_service);
    let body = WorkspaceDatabase::open(collab)?;
    Ok(Self {
      body,
      collab_service,
      database_holders: DashMap::new(),
    })
  }

  pub fn create(
    _object_id: &str,
    collab: Collab,
    collab_service: impl DatabaseCollabService,
  ) -> Result<Self, DatabaseError> {
    let collab_service = Arc::new(collab_service);
    let body = WorkspaceDatabase::create(collab);
    Ok(Self {
      body,
      collab_service,
      database_holders: DashMap::new(),
    })
  }

  pub fn close(&self) {
    self.body.close();
  }

  pub fn validate(&self) -> Result<(), DatabaseError> {
    self.body.validate()?;
    Ok(())
  }

  /// Get the database with the given database id.
  /// Return None if the database does not exist.
  pub async fn get_or_init_database(
    &self,
    database_id: &str,
  ) -> Result<Arc<RwLock<Database>>, DatabaseError> {
    // Check if the database exists in the body
    if !self.body.contains(database_id) {
      return Err(DatabaseError::DatabaseNotExist);
    }

    // Get or create holder object for this database
    let holder = self
      .database_holders
      .entry(database_id.to_string())
      .or_insert_with(|| Arc::new(DatabaseHolder::new()))
      .clone();

    // Lock the mutex and check if database is already initialized
    let mut database_guard = holder.database.lock().await;
    if let Some(database) = database_guard.as_ref() {
      trace!("Database already initialized: {}", database_id);
      return Ok(database.clone());
    }

    // Database not initialized, let's initialize it while holding the lock
    trace!("Initializing database: {}", database_id);
    let context = DatabaseContext::new(self.collab_service.clone());
    match Database::arc_open(database_id, context).await {
      Ok(database) => {
        // Store the database in the holder
        *database_guard = Some(database.clone());
        trace!("Database opened and stored: {}", database_id);
        Ok(database)
      },
      Err(err) => {
        error!("Open database failed: {}", err);
        Err(err)
      },
    }
  }

  /// Return the database id with the given view id.
  /// Multiple views can share the same database.
  pub async fn get_database_with_view_id(&self, view_id: &str) -> Option<Arc<RwLock<Database>>> {
    let database_id = self.get_database_id_with_view_id(view_id)?;
    self.get_or_init_database(&database_id).await.ok()
  }

  /// Return the database id with the given view id.
  pub fn get_database_id_with_view_id(&self, view_id: &str) -> Option<String> {
    self
      .body
      .get_database_meta_with_view_id(view_id)
      .map(|record| record.database_id)
  }

  /// Create database with inline view.
  /// The inline view is the default view of the database.
  /// If the inline view gets deleted, the database will be deleted too.
  /// So the reference views will be deleted too.
  pub async fn create_database(
    &mut self,
    params: CreateDatabaseParams,
  ) -> Result<Arc<RwLock<Database>>, DatabaseError> {
    debug_assert!(!params.database_id.is_empty());

    let context = DatabaseContext::new(self.collab_service.clone());
    // Add a new database record.
    let mut linked_views = HashSet::new();
    linked_views.extend(params.views.iter().map(|view| view.view_id.clone()));
    self
      .body
      .add_database(&params.database_id, linked_views.into_iter().collect());
    let database_id = params.database_id.clone();
    let database = Database::create_arc_with_view(params, context).await?;

    // Store in the holder
    let holder = Arc::new(DatabaseHolder::new());
    {
      let mut database_guard = holder.database.lock().await;
      *database_guard = Some(database.clone());
    }
    self.database_holders.insert(database_id, holder);

    Ok(database)
  }

  /// Create linked view that shares the same data with the inline view's database
  /// If the inline view is deleted, the reference view will be deleted too.
  pub async fn create_database_linked_view(
    &mut self,
    params: CreateViewParams,
  ) -> Result<(), DatabaseError> {
    let params = CreateViewParamsValidator::validate(params)?;
    let database = self.get_or_init_database(&params.database_id).await?;
    self.body.update_database(&params.database_id, |record| {
      // Check if the view is already linked to the database.
      if record.linked_views.contains(&params.view_id) {
        error!("The view is already linked to the database");
      } else {
        trace!("Insert linked view record: {}", params.view_id);
        record.linked_views.push(params.view_id.clone());
      }
    });

    let mut write_guard = database.write().await;
    write_guard.create_linked_view(params)
  }

  /// Delete the database with the given database id.
  pub fn delete_database(&mut self, database_id: &str) {
    self.body.delete_database(database_id);

    if let Some(persistence) = self.collab_service.persistence() {
      if let Err(err) = persistence.delete_collab(database_id) {
        error!("🔴Delete database failed: {}", err);
      }
    }
    self.database_holders.remove(database_id);
  }

  pub fn close_database(&self, database_id: &str) {
    let _ = self.database_holders.remove(database_id);
  }

  pub fn track_database(&mut self, database_id: &str, database_view_ids: Vec<String>) {
    self.body.add_database(database_id, database_view_ids);
  }

  /// Return all the database records.
  pub fn get_all_database_meta(&self) -> Vec<DatabaseMeta> {
    self.body.get_all_database_meta()
  }

  pub fn get_database_meta(&self, database_id: &str) -> Option<DatabaseMeta> {
    self.body.get_database_meta(database_id)
  }

  /// Delete the view from the database with the given view id.
  /// If the view is the inline view, the database will be deleted too.
  pub async fn delete_view(&mut self, database_id: &str, view_id: &str) {
    if let Ok(database) = self.get_or_init_database(database_id).await {
      let mut lock = database.write().await;
      lock.delete_view(view_id);
      if lock.get_all_views().is_empty() {
        drop(lock);
        // Delete the database if the view is the inline view.
        self.delete_database(database_id);
      }
    }
  }

  /// Duplicate the database that contains the view.
  pub async fn duplicate_database(
    &mut self,
    database_view_id: &str,
    new_database_view_id: &str,
  ) -> Result<Arc<RwLock<Database>>, DatabaseError> {
    let database_data = self.get_database_data(database_view_id).await?;

    let create_database_params = CreateDatabaseParams::from_database_data(
      database_data,
      database_view_id,
      new_database_view_id,
    );
    let database = self.create_database(create_database_params).await?;
    Ok(database)
  }

  /// Get all of the database data using the id of any view in the database
  pub async fn get_database_data(&self, view_id: &str) -> Result<DatabaseData, DatabaseError> {
    if let Some(database) = self.get_database_with_view_id(view_id).await {
      let data = database.read().await.get_database_data().await;
      Ok(data)
    } else {
      Err(DatabaseError::DatabaseNotExist)
    }
  }
}

impl Borrow<Collab> for WorkspaceDatabaseManager {
  #[inline]
  fn borrow(&self) -> &Collab {
    self.body.borrow()
  }
}

impl BorrowMut<Collab> for WorkspaceDatabaseManager {
  #[inline]
  fn borrow_mut(&mut self) -> &mut Collab {
    self.body.borrow_mut()
  }
}
