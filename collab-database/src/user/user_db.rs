use async_trait::async_trait;
use lru::LruCache;
use std::collections::HashMap;
use std::future::Future;
use std::num::NonZeroUsize;
use std::pin::Pin;
use std::sync::{Arc, Weak};

use collab::core::collab::{CollabDocState, MutexCollab};
use collab::preclude::updates::decoder::Decode;
use collab::preclude::{Collab, Update};
use collab_entity::CollabType;
use collab_plugins::local_storage::kv::snapshot::CollabSnapshot;

use collab_plugins::local_storage::CollabPersistenceConfig;
use collab_plugins::CollabKVDB;
use parking_lot::Mutex;
use tracing::error;

use crate::database::{Database, DatabaseContext, DatabaseData, MutexDatabase};
use crate::database_observer::DatabaseNotify;
use crate::error::DatabaseError;

use crate::user::db_record::{DatabaseViewTracker, DatabaseViewTrackerList};
use crate::views::{CreateDatabaseParams, CreateViewParams, CreateViewParamsValidator};

pub type CollabDocStateByOid = HashMap<String, CollabDocState>;
pub type CollabFuture<T> = Pin<Box<dyn Future<Output = T> + Send + Sync + 'static>>;
/// Use this trait to build a [MutexCollab] for a database object including [Database],
/// [DatabaseView], and [DatabaseRow]. When building a [MutexCollab], the caller can add
/// different [CollabPlugin]s to the [MutexCollab] to support different features.
///
#[async_trait]
pub trait DatabaseCollabService: Send + Sync + 'static {
  fn get_collab_doc_state(
    &self,
    object_id: &str,
    object_ty: CollabType,
  ) -> CollabFuture<Result<CollabDocState, DatabaseError>>;

  fn batch_get_collab_update(
    &self,
    object_ids: Vec<String>,
    object_ty: CollabType,
  ) -> CollabFuture<Result<CollabDocStateByOid, DatabaseError>>;

  fn build_collab_with_config(
    &self,
    uid: i64,
    object_id: &str,
    object_type: CollabType,
    collab_db: Weak<CollabKVDB>,
    collab_doc_state: CollabDocState,
    config: CollabPersistenceConfig,
  ) -> Arc<MutexCollab>;
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
  uid: i64,
  collab: Arc<MutexCollab>,
  collab_db: Weak<CollabKVDB>,
  config: CollabPersistenceConfig,
  collab_service: Arc<dyn DatabaseCollabService>,
  /// In memory database handlers.
  /// The key is the database id. The handler will be added when the database is opened or created.
  /// and the handler will be removed when the database is deleted or closed.
  open_handlers: Mutex<LruCache<String, Arc<MutexDatabase>>>,
}

impl WorkspaceDatabase {
  pub fn open<T>(
    uid: i64,
    collab: Arc<MutexCollab>,
    collab_db: Weak<CollabKVDB>,
    config: CollabPersistenceConfig,
    collab_service: T,
  ) -> Self
  where
    T: DatabaseCollabService,
  {
    let collab_service = Arc::new(collab_service);
    let collab_guard = collab.lock();
    drop(collab_guard);

    let open_handlers = Mutex::new(LruCache::new(NonZeroUsize::new(5).unwrap()));
    Self {
      uid,
      collab_db,
      collab,
      open_handlers,
      config,
      collab_service,
    }
  }

  /// Get the database with the given database id.
  /// Return None if the database does not exist.
  pub async fn get_database(&self, database_id: &str) -> Option<Arc<MutexDatabase>> {
    if !self.database_tracker_list().contains(database_id) {
      return None;
    }
    let database = self.open_handlers.lock().get(database_id).cloned();
    let collab_db = self.collab_db.upgrade()?;
    match database {
      None => {
        let mut collab_doc_state = CollabDocState::default();
        let is_exist = collab_db
          .is_exist(self.uid, database_id)
          .await
          .unwrap_or(false);
        if !is_exist {
          // Try to load the database from the remote. The database doesn't exist in the local only
          // when the user has deleted the database or the database is using a remote storage.
          match self
            .collab_service
            .get_collab_doc_state(database_id, CollabType::Database)
            .await
          {
            Ok(fetched_doc_state) => {
              if fetched_doc_state.is_empty() {
                tracing::error!("Failed to get updates for database: {}", database_id);
                return None;
              }
              collab_doc_state = fetched_doc_state;
            },
            Err(e) => {
              tracing::error!("Failed to get collab updates for database: {}", e);
              return None;
            },
          }
        }
        let notifier = DatabaseNotify::default();
        let collab = self.collab_for_database(database_id, collab_doc_state);
        let context = DatabaseContext {
          uid: self.uid,
          db: self.collab_db.clone(),
          collab,
          collab_service: self.collab_service.clone(),
          notifier: Some(notifier),
        };
        let database = Database::get_or_create(database_id, context).ok()?;

        // The database is not exist in local disk, which means the rows of the database are not
        // loaded yet. Load the rows from the remote with limit 100.
        if !is_exist {
          database.load_all_rows();
        }

        let database = Arc::new(MutexDatabase::new(database));
        self
          .open_handlers
          .lock()
          .put(database_id.to_string(), database.clone());
        Some(database)
      },
      Some(database) => Some(database),
    }
  }
  /// Return the database id with the given view id.
  /// Multiple views can share the same database.
  pub async fn get_database_with_view_id(&self, view_id: &str) -> Option<Arc<MutexDatabase>> {
    let database_id = self.get_database_id_with_view_id(view_id)?;
    self.get_database(&database_id).await
  }

  /// Return the database id with the given view id.
  pub fn get_database_id_with_view_id(&self, view_id: &str) -> Option<String> {
    self
      .database_tracker_list()
      .get_database_view_tracker_with_view_id(view_id)
      .map(|record| record.database_id)
  }

  /// Create database with inline view.
  /// The inline view is the default view of the database.
  /// If the inline view gets deleted, the database will be deleted too.
  /// So the reference views will be deleted too.
  pub fn create_database(
    &self,
    params: CreateDatabaseParams,
  ) -> Result<Arc<MutexDatabase>, DatabaseError> {
    debug_assert!(!params.database_id.is_empty());
    debug_assert!(!params.view_id.is_empty());

    // Create a [Collab] for the given database id.
    let collab = self.collab_for_database(&params.database_id, CollabDocState::default());
    let notifier = DatabaseNotify::default();
    let context = DatabaseContext {
      uid: self.uid,
      db: self.collab_db.clone(),
      collab,
      collab_service: self.collab_service.clone(),
      notifier: Some(notifier),
    };

    // Add a new database record.
    self
      .database_tracker_list()
      .add_database(&params.database_id, vec![params.view_id.clone()]);
    let database_id = params.database_id.clone();
    // TODO(RS): insert the first view of the database.
    let mutex_database = MutexDatabase::new(Database::create_with_inline_view(params, context)?);
    let database = Arc::new(mutex_database);
    self.open_handlers.lock().put(database_id, database.clone());
    Ok(database)
  }

  pub fn track_database(&self, database_id: &str, database_view_ids: Vec<String>) {
    self
      .database_tracker_list()
      .add_database(database_id, database_view_ids);
  }

  /// Create database with the data duplicated from the given database.
  /// The [DatabaseData] contains all the database data. It can be
  /// used to restore the database from the backup.
  pub fn create_database_with_data(
    &self,
    data: DatabaseData,
  ) -> Result<Arc<MutexDatabase>, DatabaseError> {
    let DatabaseData { view, fields, rows } = data;
    let params = CreateDatabaseParams::from_view(view, fields, rows);
    let database = self.create_database(params)?;
    Ok(database)
  }

  /// Create linked view that shares the same data with the inline view's database
  /// If the inline view is deleted, the reference view will be deleted too.
  pub async fn create_database_linked_view(
    &self,
    params: CreateViewParams,
  ) -> Result<(), DatabaseError> {
    let params = CreateViewParamsValidator::validate(params)?;
    if let Some(database) = self.get_database(&params.database_id).await {
      self
        .database_tracker_list()
        .update_database(&params.database_id, |record| {
          record.linked_views.insert(params.view_id.clone());
        });
      database.lock().await.create_linked_view(params)
    } else {
      Err(DatabaseError::DatabaseNotExist)
    }
  }

  /// Delete the database with the given database id.
  pub async fn delete_database(&self, database_id: &str) {
    self.database_tracker_list().delete_database(database_id);
    if let Some(collab_db) = self.collab_db.upgrade() {
      if let Err(err) = collab_db.delete_doc(self.uid, database_id).await {
        error!("Delete database failed: {}", err);
      }
    }

    let database = self.open_handlers.lock().pop(database_id);
    if let Some(database) = database {
      if let Ok(lock_guard) = database.try_lock() {
        lock_guard.close();
      }
    }
  }

  /// Close the database with the given database id.
  pub async fn close_database(&self, database_id: &str) {
    let database = self.open_handlers.lock().pop(database_id);
    if let Some(database) = database {
      database.lock().await.close();
    }
  }

  /// Return all the database records.
  pub fn get_all_databases(&self) -> Vec<DatabaseViewTracker> {
    self.database_tracker_list().get_all_database_tracker()
  }

  pub fn restore_database_from_snapshot(
    &self,
    database_id: &str,
    snapshot: CollabSnapshot,
  ) -> Result<Database, DatabaseError> {
    let collab = self.collab_for_database(database_id, CollabDocState::default());
    let update =
      Update::decode_v1(&snapshot.data).map_err(|err| DatabaseError::Internal(err.into()))?;
    collab.lock().with_origin_transact_mut(|txn| {
      txn.apply_update(update);
    });

    let context = DatabaseContext {
      uid: self.uid,
      db: self.collab_db.clone(),
      collab,
      collab_service: self.collab_service.clone(),
      notifier: None,
    };
    Database::get_or_create(database_id, context)
  }

  /// Delete the view from the database with the given view id.
  /// If the view is the inline view, the database will be deleted too.
  pub async fn delete_view(&self, database_id: &str, view_id: &str) {
    if let Some(database) = self.get_database(database_id).await {
      database.lock().await.delete_view(view_id);
      let is_inline_view = database.lock().await.is_inline_view(view_id);
      if is_inline_view {
        // Delete the database if the view is the inline view.
        self.delete_database(database_id).await;
      }
    }
  }

  /// Duplicate the database that contains the view.
  pub async fn duplicate_database(
    &self,
    view_id: &str,
  ) -> Result<Arc<MutexDatabase>, DatabaseError> {
    let DatabaseData { view, fields, rows } = self.get_database_duplicated_data(view_id).await?;
    let params = CreateDatabaseParams::from_view(view, fields, rows);
    let database = self.create_database(params)?;
    Ok(database)
  }

  /// Duplicate the database with the given view id.
  pub async fn get_database_duplicated_data(
    &self,
    view_id: &str,
  ) -> Result<DatabaseData, DatabaseError> {
    if let Some(database) = self.get_database_with_view_id(view_id).await {
      let data = database.lock().await.duplicate_database().await;
      Ok(data)
    } else {
      Err(DatabaseError::DatabaseNotExist)
    }
  }

  /// Create a new [Collab] instance for given database id.
  fn collab_for_database(&self, database_id: &str, doc_state: CollabDocState) -> Arc<MutexCollab> {
    self.collab_service.build_collab_with_config(
      self.uid,
      database_id,
      CollabType::Database,
      self.collab_db.clone(),
      doc_state,
      self.config.clone(),
    )
  }

  fn database_tracker_list(&self) -> DatabaseViewTrackerList {
    DatabaseViewTrackerList::from_collab(&self.collab.lock())
  }
}

pub fn get_all_database_view_trackers(collab: &Collab) -> Vec<DatabaseViewTracker> {
  DatabaseViewTrackerList::from_collab(collab).get_all_database_tracker()
}
