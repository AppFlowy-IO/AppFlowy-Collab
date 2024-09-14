use crate::database::{try_fixing_database, Database, DatabaseContext, DatabaseData};

use crate::error::DatabaseError;
use crate::workspace_database::body::{DatabaseMeta, WorkspaceDatabaseBody};
use async_trait::async_trait;
use collab::core::collab::DataSource;
use collab::preclude::Collab;
use collab_entity::CollabType;

use collab::entity::EncodedCollab;

use crate::entity::{CreateDatabaseParams, CreateViewParams, CreateViewParamsValidator};
use crate::rows::RowId;
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
use tracing::{error, info};

pub type EncodeCollabByOid = HashMap<String, EncodedCollab>;
pub type DataSourceByOid = HashMap<String, DataSource>;

/// Use this trait to build a [MutexCollab] for a database object including [Database],
/// [DatabaseView], and [DatabaseRow]. When building a [MutexCollab], the caller can add
/// different [CollabPlugin]s to the [MutexCollab] to support different features.
///
#[async_trait]
pub trait DatabaseCollabService: Send + Sync + 'static {
  async fn build_collab(
    &self,
    object_id: &str,
    object_type: CollabType,
    encoded_collab: Option<(EncodedCollab, bool)>,
  ) -> Result<Collab, DatabaseError>;

  async fn get_collabs(
    &self,
    object_ids: Vec<String>,
    collab_type: CollabType,
  ) -> Result<EncodeCollabByOid, DatabaseError>;

  fn persistence(&self) -> Option<Arc<dyn DatabaseCollabPersistenceService>>;
}

pub struct NoPersistenceDatabaseCollabService;
#[async_trait]
impl DatabaseCollabService for NoPersistenceDatabaseCollabService {
  async fn build_collab(
    &self,
    object_id: &str,
    _object_type: CollabType,
    encoded_collab: Option<(EncodedCollab, bool)>,
  ) -> Result<Collab, DatabaseError> {
    match encoded_collab {
      None => Collab::new_with_source(
        CollabOrigin::Empty,
        object_id,
        CollabPersistenceImpl {
          persistence: self.persistence(),
        }
        .into(),
        vec![],
        false,
      )
      .map_err(|err| DatabaseError::Internal(err.into())),
      Some((encoded_collab, _)) => Collab::new_with_source(
        CollabOrigin::Empty,
        object_id,
        encoded_collab.into(),
        vec![],
        false,
      )
      .map_err(|err| DatabaseError::Internal(err.into())),
    }
  }

  async fn get_collabs(
    &self,
    object_ids: Vec<String>,
    collab_type: CollabType,
  ) -> Result<EncodeCollabByOid, DatabaseError> {
    let map: HashMap<String, _> = object_ids
      .into_par_iter()
      .filter_map(|object_id| {
        let persistence = self.persistence();

        let result = Collab::new_with_source(
          CollabOrigin::Empty,
          &object_id,
          CollabPersistenceImpl { persistence }.into(),
          vec![],
          false,
        )
        .map_err(|err| DatabaseError::Internal(err.into()))
        .and_then(|collab| {
          collab
            .encode_collab_v1(|collab| {
              collab_type.validate_require_data(collab)?;
              Ok(())
            })
            .map_err(DatabaseError::Internal)
        });

        // If successful, return the object ID and the encoded collab
        match result {
          Ok(encoded_collab) => Some((object_id, encoded_collab)),
          Err(_) => None, // Ignore errors, but you can log them if necessary
        }
      })
      .collect();

    Ok(map)
  }

  fn persistence(&self) -> Option<Arc<dyn DatabaseCollabPersistenceService>> {
    None
  }
}

pub trait DatabaseCollabPersistenceService: Send + Sync + 'static {
  fn load_collab(&self, collab: &mut Collab);

  fn get_encoded_collab(&self, object_id: &str, collab_type: CollabType) -> Option<EncodedCollab>;

  fn delete_collab(&self, object_id: &str) -> Result<(), DatabaseError>;

  fn save_collab(
    &self,
    object_id: &str,
    encoded_collab: EncodedCollab,
  ) -> Result<(), DatabaseError>;

  fn is_collab_exist(&self, object_id: &str) -> bool;

  fn flush_collabs(
    &self,
    encoded_collabs: Vec<(String, EncodedCollab)>,
  ) -> Result<(), DatabaseError>;

  fn is_row_exist_partition(&self, row_ids: Vec<RowId>) -> (Vec<RowId>, Vec<RowId>);
}

pub struct CollabPersistenceImpl {
  pub persistence: Option<Arc<dyn DatabaseCollabPersistenceService>>,
}
impl CollabPersistence for CollabPersistenceImpl {
  fn load_collab_from_disk(&self, collab: &mut Collab) {
    if let Some(persistence) = &self.persistence {
      persistence.load_collab(collab);
    }
  }

  fn save_collab_to_disk(
    &self,
    object_id: &str,
    encoded_collab: EncodedCollab,
  ) -> Result<(), CollabError> {
    if let Some(persistence) = &self.persistence {
      persistence
        .save_collab(object_id, encoded_collab)
        .map_err(|err| CollabError::Internal(anyhow!(err)))
    } else {
      Err(CollabError::Internal(anyhow!(
        "collab persistence is not found"
      )))
    }
  }
}

impl From<CollabPersistenceImpl> for DataSource {
  fn from(persistence: CollabPersistenceImpl) -> Self {
    DataSource::Disk(Some(Box::new(persistence)))
  }
}

/// A [WorkspaceDatabase] indexes the databases within a workspace.
/// Within a workspace, the view ID is used to identify each database. Therefore, you can use the view_id to retrieve
/// the actual database ID from [WorkspaceDatabase]. Additionally, [WorkspaceDatabase] allows you to obtain a database
/// using its database ID.
///
/// Relation between database ID and view ID:
/// One database ID can have multiple view IDs.
///
pub struct WorkspaceDatabase {
  object_id: String,
  collab: Collab,
  body: WorkspaceDatabaseBody,
  collab_service: Arc<dyn DatabaseCollabService>,
  /// In memory database handlers.
  /// The key is the database id. The handler will be added when the database is opened or created.
  /// and the handler will be removed when the database is deleted or closed.
  databases: DashMap<String, Arc<RwLock<Database>>>,
}

impl WorkspaceDatabase {
  pub fn open(
    object_id: &str,
    mut collab: Collab,
    collab_service: impl DatabaseCollabService,
  ) -> Self {
    let collab_service = Arc::new(collab_service);
    let body = WorkspaceDatabaseBody::open(&mut collab);

    Self {
      object_id: object_id.to_string(),
      collab,
      body,
      collab_service,
      databases: DashMap::new(),
    }
  }

  pub fn close(&self) {
    self.collab.remove_all_plugins();
  }

  pub fn validate(&self) -> Result<(), DatabaseError> {
    CollabType::WorkspaceDatabase.validate_require_data(&self.collab)?;
    Ok(())
  }

  /// Get the database with the given database id.
  /// Return None if the database does not exist.
  // The original function, now using the extracted fix_and_open_database function
  pub async fn get_or_init_database(
    &self,
    database_id: &str,
  ) -> Result<Arc<RwLock<Database>>, DatabaseError> {
    // Check if the database exists in the body
    if !self.body.contains(&self.collab.transact(), database_id) {
      return Err(DatabaseError::DatabaseNotExist);
    }

    // Check if the database is already initialized and cached
    if let Some(database) = self.databases.get(database_id).as_deref().cloned() {
      return Ok(database);
    }

    // Helper function to insert the database into the cache
    let insert_database = |db: Database| -> Arc<RwLock<Database>> {
      let database = Arc::new(RwLock::new(db));
      self
        .databases
        .insert(database_id.to_string(), database.clone());
      database
    };

    // Try to open the database
    let context = DatabaseContext::new(self.collab_service.clone());
    match Database::open(database_id, context).await {
      Ok(database) => Ok(insert_database(database)),
      // If the database is missing required data, try to fix it and open it again
      Err(err) => {
        if err.is_no_required_data() {
          if self
            .fix_and_open_database(
              database_id,
              DatabaseContext::new(self.collab_service.clone()),
            )
            .await
            .is_ok()
          {
            if let Ok(database) = Database::open(
              database_id,
              DatabaseContext::new(self.collab_service.clone()),
            )
            .await
            {
              return Ok(insert_database(database));
            }
          }
          Err(err)
        } else {
          error!("Open database failed: {}", err);
          Err(err)
        }
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
    let txn = self.collab.transact();
    self
      .body
      .get_database_meta_with_view_id(&txn, view_id)
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
    linked_views.insert(params.inline_view_id.to_string());
    linked_views.extend(
      params
        .views
        .iter()
        .filter(|view| view.view_id != params.inline_view_id)
        .map(|view| view.view_id.clone()),
    );
    let mut txn = self.collab.transact_mut();
    self.body.add_database(
      &mut txn,
      &params.database_id,
      linked_views.into_iter().collect(),
    );
    let database_id = params.database_id.clone();
    let database = Database::create_with_view(params, context).await.unwrap();
    let mutex_database = RwLock::from(database);
    let database = Arc::new(mutex_database);
    self.databases.insert(database_id, database.clone());
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
    let mut txn = self.collab.transact_mut();
    self
      .body
      .update_database(&mut txn, &params.database_id, |record| {
        // Check if the view is already linked to the database.
        if record.linked_views.contains(&params.view_id) {
          error!("The view is already linked to the database");
        } else {
          record.linked_views.push(params.view_id.clone());
        }
      });

    let mut write_guard = database.write().await;
    write_guard.create_linked_view(params)
  }

  /// Delete the database with the given database id.
  pub fn delete_database(&mut self, database_id: &str) {
    let mut txn = self.collab.transact_mut();
    self.body.delete_database(&mut txn, database_id);
    drop(txn);

    if let Some(persistence) = self.collab_service.persistence() {
      if let Err(err) = persistence.delete_collab(database_id) {
        error!("🔴Delete database failed: {}", err);
      }
    }
    self.databases.remove(database_id);
  }

  pub fn close_database(&self, database_id: &str) {
    let _ = self.databases.remove(database_id);
  }

  pub fn track_database(&mut self, database_id: &str, database_view_ids: Vec<String>) {
    let mut txn = self.collab.transact_mut();
    self
      .body
      .add_database(&mut txn, database_id, database_view_ids);
  }

  /// Return all the database records.
  pub fn get_all_database_meta(&self) -> Vec<DatabaseMeta> {
    let txn = self.collab.transact();
    self.body.get_all_database_meta(&txn)
  }

  pub fn get_database_meta(&self, database_id: &str) -> Option<DatabaseMeta> {
    let txn = self.collab.transact();
    self.body.get_database_meta(&txn, database_id)
  }

  /// Delete the view from the database with the given view id.
  /// If the view is the inline view, the database will be deleted too.
  pub async fn delete_view(&mut self, database_id: &str, view_id: &str) {
    if let Ok(database) = self.get_or_init_database(database_id).await {
      let mut lock = database.write().await;
      lock.delete_view(view_id);
      if lock.is_inline_view(view_id) {
        drop(lock);
        // Delete the database if the view is the inline view.
        self.delete_database(database_id);
      }
    }
  }

  /// Duplicate the database that contains the view.
  pub async fn duplicate_database(
    &mut self,
    view_id: &str,
  ) -> Result<Arc<RwLock<Database>>, DatabaseError> {
    let database_data = self.get_database_data(view_id).await?;

    let create_database_params = CreateDatabaseParams::from_database_data(database_data, None);
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

  pub fn flush_workspace_database(&self) -> Result<(), DatabaseError> {
    let encode_collab = self
      .collab
      .encode_collab_v1(|collab| CollabType::WorkspaceDatabase.validate_require_data(collab))?;
    self
      .collab_service
      .persistence()
      .ok_or_else(|| DatabaseError::Internal(anyhow!("collab persistence is not found")))?
      .flush_collabs(vec![(self.object_id.clone(), encode_collab)])?;
    Ok(())
  }

  async fn fix_and_open_database(
    &self,
    database_id: &str,
    context: DatabaseContext,
  ) -> Result<(), DatabaseError> {
    // Try to get the database metadata
    info!("[Fix]: Attempting to fix database: {}", database_id);
    if let Some(database_meta) = self.get_database_meta(database_id) {
      if let Ok(mut collab) = context
        .collab_service
        .build_collab(database_id, CollabType::Database, None)
        .await
      {
        // Attempt to fix the database inline view ID
        if try_fixing_database(&mut collab, database_meta).is_ok() {
          info!("[Fix]: database:{} by adding inline view", database_id);
          // Retry opening the database after attempting to fix it
          match Database::open(database_id, context).await {
            Ok(database) => {
              if let Some(persistence) = self.collab_service.persistence() {
                match database.encode_collab_v1(|collab| {
                  CollabType::Database.validate_require_data(collab)?;
                  Ok::<_, DatabaseError>(())
                }) {
                  Ok(encoded_collab) => {
                    info!("[Fix]: save database:{} to disk", database_id);
                    persistence.save_collab(database_id, encoded_collab).ok();
                  },
                  Err(err) => {
                    error!("[Fix]: fix database:{} failed: {}", database_id, err);
                  },
                }
              }
              return Ok(());
            },
            Err(err) => {
              info!("[Fix]: fix database:{} failed: {}", database_id, err);
            },
          }
        }
      }
    } else {
      info!("Can't find any database meta for database: {}", database_id);
    }

    Err(DatabaseError::Internal(anyhow!("Can't fix the database")))
  }
}

impl Borrow<Collab> for WorkspaceDatabase {
  #[inline]
  fn borrow(&self) -> &Collab {
    &self.collab
  }
}

impl BorrowMut<Collab> for WorkspaceDatabase {
  #[inline]
  fn borrow_mut(&mut self) -> &mut Collab {
    &mut self.collab
  }
}
