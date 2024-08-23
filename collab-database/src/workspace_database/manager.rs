use crate::database::{Database, DatabaseContext, DatabaseData};
use crate::database_state::DatabaseNotify;
use crate::error::DatabaseError;
use crate::workspace_database::database_meta::{DatabaseMeta, DatabaseMetaList};
use async_trait::async_trait;
use collab::core::collab::DataSource;
use collab::preclude::Collab;
use collab_entity::CollabType;

use collab::entity::EncodedCollab;

use dashmap::DashMap;
use std::borrow::{Borrow, BorrowMut};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::entity::{CreateDatabaseParams, CreateViewParams, CreateViewParamsValidator};
use crate::rows::RowId;
use collab::core::collab_plugin::CollabPersistence;
use tokio::sync::RwLock;
use tracing::error;

pub type EncodeCollabByOid = HashMap<String, EncodedCollab>;

/// Use this trait to build a [MutexCollab] for a database object including [Database],
/// [DatabaseView], and [DatabaseRow]. When building a [MutexCollab], the caller can add
/// different [CollabPlugin]s to the [MutexCollab] to support different features.
///
#[async_trait]
pub trait DatabaseCollabService: Send + Sync + 'static {
  async fn get_encode_collab(
    &self,
    object_id: &str,
    object_ty: CollabType,
  ) -> Result<Option<EncodedCollab>, DatabaseError>;

  async fn batch_get_encode_collab(
    &self,
    object_ids: Vec<String>,
    object_ty: CollabType,
  ) -> Result<EncodeCollabByOid, DatabaseError>;

  fn build_collab(
    &self,
    object_id: &str,
    object_type: CollabType,
    data_source: DataSource,
  ) -> Result<Collab, DatabaseError>;

  fn persistence(&self) -> Option<Box<dyn DatabaseCollabPersistenceService>>;
}

pub trait DatabaseCollabPersistenceService: Send + Sync + 'static {
  fn load_collab(&self, collab: &mut Collab);

  fn delete_collab(&self, object_id: &str) -> Result<(), DatabaseError>;

  fn is_collab_exist(&self, object_id: &str) -> bool;

  fn flush_collab(
    &self,
    object_id: &str,
    encode_collab: EncodedCollab,
  ) -> Result<(), DatabaseError>;

  fn is_row_exist_partition(&self, row_ids: Vec<RowId>) -> (Vec<RowId>, Vec<RowId>);
}

pub struct CollabPersistenceImpl {
  pub persistence: Option<Box<dyn DatabaseCollabPersistenceService>>,
}
impl CollabPersistence for CollabPersistenceImpl {
  fn load_collab_from_disk(&self, collab: &mut Collab) {
    if let Some(persistence) = &self.persistence {
      persistence.load_collab(collab);
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
  collab: Collab,
  meta_list: DatabaseMetaList,
  collab_service: Arc<dyn DatabaseCollabService>,
  /// In memory database handlers.
  /// The key is the database id. The handler will be added when the database is opened or created.
  /// and the handler will be removed when the database is deleted or closed.
  databases: DashMap<String, Arc<RwLock<Database>>>,
}

impl WorkspaceDatabase {
  pub fn open<T>(mut collab: Collab, collab_service: T) -> Self
  where
    T: DatabaseCollabService,
  {
    let collab_service = Arc::new(collab_service);
    let meta_list = DatabaseMetaList::new(&mut collab);

    Self {
      collab,
      meta_list,
      collab_service,
      databases: DashMap::new(),
    }
  }

  pub fn close(&self) {
    self.collab.clear_plugins();
  }

  pub fn validate(&self) -> Result<(), DatabaseError> {
    CollabType::WorkspaceDatabase
      .validate_require_data(&self.collab)
      .map_err(|_| DatabaseError::NoRequiredData)?;
    Ok(())
  }

  pub(crate) async fn get_database_collab(&self, database_id: &str) -> Option<Collab> {
    let mut data_source: DataSource = CollabPersistenceImpl {
      persistence: self.collab_service.persistence(),
    }
    .into();

    let is_exist = self
      .collab_service
      .persistence()
      .map(|persistence| persistence.is_collab_exist(database_id))
      .unwrap_or(false);

    if !is_exist {
      // Try to load the database from the remote. The database doesn't exist in the local only
      // when the user has deleted the database or the database is using a remote storage.
      match self
        .collab_service
        .get_encode_collab(database_id, CollabType::Database)
        .await
      {
        Ok(Some(encode_collab)) => {
          if encode_collab.doc_state.is_empty() {
            error!("Failed to get updates for database: {}", database_id);
            return None;
          }

          data_source = DataSource::from(encode_collab);
        },
        Ok(None) => {
          // do nothing
        },
        Err(e) => {
          error!("Failed to get collab updates for database: {}", e);
          return None;
        },
      }
    }
    let database_collab = self.collab_for_database(database_id, data_source).ok()?;
    Some(database_collab)
  }

  /// Get the database with the given database id.
  /// Return None if the database does not exist.
  pub async fn get_or_create_database(&self, database_id: &str) -> Option<Arc<RwLock<Database>>> {
    if !self
      .meta_list
      .contains(&self.collab.transact(), database_id)
    {
      return None;
    }

    let database = self.databases.get(database_id).as_deref().cloned();
    match database {
      None => {
        // If the database is not exist, create a new one.
        let _notifier = DatabaseNotify::default();
        let is_exist = self
          .collab_service
          .persistence()?
          .is_collab_exist(database_id);
        let collab = self.get_database_collab(database_id).await?;

        let context = DatabaseContext::new(collab, self.collab_service.clone());
        let database = Database::open(database_id, context).ok()?;
        // The database is not exist in local disk, which means the rows of the database are not
        // loaded yet.
        if !is_exist {
          database.load_first_screen_rows().await;
        }

        // Create a new [MutexDatabase] and add it to the databases.
        let database = Arc::new(RwLock::new(database));
        self
          .databases
          .insert(database_id.to_string(), database.clone());
        Some(database)
      },
      Some(database) => Some(database),
    }
  }

  /// Return the database id with the given view id.
  /// Multiple views can share the same database.
  pub async fn get_database_with_view_id(&self, view_id: &str) -> Option<Arc<RwLock<Database>>> {
    let database_id = self.get_database_id_with_view_id(view_id)?;
    self.get_or_create_database(&database_id).await
  }

  /// Return the database id with the given view id.
  pub fn get_database_id_with_view_id(&self, view_id: &str) -> Option<String> {
    let txn = self.collab.transact();
    self
      .meta_list
      .get_database_meta_with_view_id(&txn, view_id)
      .map(|record| record.database_id)
  }

  /// Create database with inline view.
  /// The inline view is the default view of the database.
  /// If the inline view gets deleted, the database will be deleted too.
  /// So the reference views will be deleted too.
  pub fn create_database(
    &mut self,
    params: CreateDatabaseParams,
  ) -> Result<Arc<RwLock<Database>>, DatabaseError> {
    debug_assert!(!params.database_id.is_empty());

    // Create a [Collab] for the given database id.
    let data_source: DataSource = CollabPersistenceImpl {
      persistence: self.collab_service.persistence(),
    }
    .into();
    let collab = self.collab_for_database(&params.database_id, data_source)?;
    let context = DatabaseContext::new(collab, self.collab_service.clone());

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
    self.meta_list.add_database(
      &mut txn,
      &params.database_id,
      linked_views.into_iter().collect(),
    );
    let database_id = params.database_id.clone();

    let database = futures::executor::block_on(async {
      Database::create_with_view(params, context).await.unwrap()
    });

    let mutex_database = RwLock::new(database);
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
    if let Some(database) = self.get_or_create_database(&params.database_id).await {
      let mut txn = self.collab.transact_mut();
      self
        .meta_list
        .update_database(&mut txn, &params.database_id, |record| {
          // Check if the view is already linked to the database.
          if record.linked_views.contains(&params.view_id) {
            error!("The view is already linked to the database");
          } else {
            record.linked_views.push(params.view_id.clone());
          }
        });
      database.write().await.create_linked_view(params)
    } else {
      Err(DatabaseError::DatabaseNotExist)
    }
  }

  /// Delete the database with the given database id.
  pub fn delete_database(&mut self, database_id: &str) {
    let mut txn = self.collab.transact_mut();
    self.meta_list.delete_database(&mut txn, database_id);
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
      .meta_list
      .add_database(&mut txn, database_id, database_view_ids);
  }

  /// Return all the database records.
  pub fn get_all_database_meta(&self) -> Vec<DatabaseMeta> {
    let txn = self.collab.transact();
    self.meta_list.get_all_database_meta(&txn)
  }

  /// Delete the view from the database with the given view id.
  /// If the view is the inline view, the database will be deleted too.
  pub async fn delete_view(&mut self, database_id: &str, view_id: &str) {
    if let Some(database) = self.get_or_create_database(database_id).await {
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

    let create_database_params = CreateDatabaseParams::from_database_data(database_data);
    let database = self.create_database(create_database_params)?;
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

  /// Create a new [Collab] instance for given database id.
  fn collab_for_database(
    &self,
    database_id: &str,
    data_source: DataSource,
  ) -> Result<Collab, DatabaseError> {
    self
      .collab_service
      .build_collab(database_id, CollabType::Database, data_source)
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
