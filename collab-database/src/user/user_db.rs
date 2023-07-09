use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use collab::core::collab::{CollabRawData, MutexCollab};
use collab::preclude::updates::decoder::Decode;
use collab::preclude::{lib0Any, Collab, MapPrelim, Update};
use collab_persistence::doc::YrsDocAction;
use collab_persistence::kv::rocks_kv::RocksCollabDB;
use collab_persistence::snapshot::{CollabSnapshot, SnapshotAction};
use collab_plugins::disk::rocksdb::CollabPersistenceConfig;
use parking_lot::RwLock;

use crate::blocks::{Block, BlockEvent};
use crate::database::{Database, DatabaseContext, DatabaseData, MutexDatabase};
use crate::error::DatabaseError;
use crate::user::db_record::{DatabaseArray, DatabaseRecord};
use crate::views::{CreateDatabaseParams, CreateViewParams, CreateViewParamsValidator};

pub type CollabFuture<T> = Pin<Box<dyn Future<Output = T> + Send + Sync + 'static>>;
/// Use this trait to build a [MutexCollab] for a database object including [Database],
/// [DatabaseView], and [DatabaseRow]. When building a [MutexCollab], the caller can add
/// different [CollabPlugin]s to the [MutexCollab] to support different features.
///
pub trait DatabaseCollabService: Send + Sync + 'static {
  fn get_collab_updates(
    &self,
    object_id: &str,
  ) -> CollabFuture<Result<Vec<Vec<u8>>, DatabaseError>>;

  fn build_collab_with_config(
    &self,
    uid: i64,
    object_id: &str,
    object_name: &str,
    collab_db: Arc<RocksCollabDB>,
    collab_raw_data: CollabRawData,
    config: &CollabPersistenceConfig,
  ) -> Arc<MutexCollab>;
}

/// A [WorkspaceDatabase] is used to index all the databases of a user.
pub struct WorkspaceDatabase {
  uid: i64,
  collab: Arc<MutexCollab>,
  collab_db: Arc<RocksCollabDB>,
  /// It used to keep track of the blocks. Each block contains a list of [Row]s
  /// A database rows will be stored in multiple blocks.
  block: Block,
  config: CollabPersistenceConfig,
  collab_service: Arc<dyn DatabaseCollabService>,
  /// In memory database handlers.
  /// The key is the database id. The handler will be added when the database is opened or created.
  /// and the handler will be removed when the database is deleted or closed.
  open_handlers: RwLock<HashMap<String, Arc<MutexDatabase>>>,
}

const DATABASES: &str = "databases";

pub fn make_workspace_database_id(uid: i64) -> String {
  STANDARD.encode(format!("{}:user:database", uid))
}

impl WorkspaceDatabase {
  pub fn open<T>(
    uid: i64,
    collab: Arc<MutexCollab>,
    collab_db: Arc<RocksCollabDB>,
    config: CollabPersistenceConfig,
    collab_service: T,
  ) -> Self
  where
    T: DatabaseCollabService,
  {
    let collab_service = Arc::new(collab_service);
    let collab_guard = collab.lock();
    let _ = get_database_array_ref(&collab_guard);

    let block = Block::new(uid, collab_db.clone(), collab_service.clone());
    drop(collab_guard);

    Self {
      uid,
      collab_db,
      collab,
      block,
      open_handlers: Default::default(),
      config,
      collab_service,
    }
  }

  pub fn subscribe_block_event(&self) -> tokio::sync::broadcast::Receiver<BlockEvent> {
    self.block.subscribe_event()
  }

  /// Get the database with the given database id.
  /// Return None if the database does not exist.
  pub async fn get_database(&self, database_id: &str) -> Option<Arc<MutexDatabase>> {
    if !self.database_array().contains(database_id) {
      return None;
    }
    let database = self.open_handlers.read().get(database_id).cloned();
    match database {
      None => {
        let mut collab_raw_data = CollabRawData::default();
        let is_exist = self.collab_db.read_txn().is_exist(self.uid, &database_id);
        if !is_exist {
          match self.collab_service.get_collab_updates(database_id).await {
            Ok(updates) => collab_raw_data = updates,
            Err(e) => {
              tracing::error!("Failed to get collab updates for database: {}", e);
              return None;
            },
          }
        }

        let blocks = self.block.clone();
        let collab = self.collab_for_database(database_id, collab_raw_data);
        let context = DatabaseContext {
          collab,
          block: blocks,
          collab_builder: self.collab_service.clone(),
        };
        let database = Arc::new(MutexDatabase::new(
          Database::get_or_create(database_id, context).ok()?,
        ));
        self
          .open_handlers
          .write()
          .insert(database_id.to_string(), database.clone());
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
      .database_array()
      .get_database_record_with_view_id(view_id)
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
    let collab = self.collab_for_database(&params.database_id, CollabRawData::default());
    let blocks = self.block.clone();
    let context = DatabaseContext {
      collab,
      block: blocks,
      collab_builder: self.collab_service.clone(),
    };

    // Add a new database record.
    self
      .database_array()
      .add_database(&params.database_id, &params.view_id, &params.name);
    let database_id = params.database_id.clone();
    let mutex_database = MutexDatabase::new(Database::create_with_inline_view(params, context)?);
    let database = Arc::new(mutex_database);
    self
      .open_handlers
      .write()
      .insert(database_id, database.clone());
    Ok(database)
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
        .database_array()
        .update_database(&params.database_id, |record| {
          record.views.insert(params.view_id.clone());
        });
      database.lock().create_linked_view(params)
    } else {
      Err(DatabaseError::DatabaseNotExist)
    }
  }

  /// Delete the database with the given database id.
  pub fn delete_database(&self, database_id: &str) {
    self.database_array().delete_database(database_id);
    let _ = self.collab_db.with_write_txn(|w_db_txn| {
      match w_db_txn.delete_doc(self.uid, database_id) {
        Ok(_) => {},
        Err(err) => tracing::error!("🔴Delete database failed: {}", err),
      }
      Ok(())
    });
    self.open_handlers.write().remove(database_id);
  }

  /// Close the database with the given database id.
  pub fn close_database(&self, database_id: &str) {
    self.open_handlers.write().remove(database_id);
  }

  /// Return all the database records.
  pub fn get_all_databases(&self) -> Vec<DatabaseRecord> {
    self.database_array().get_all_databases()
  }

  pub fn get_database_snapshots(&self, database_id: &str) -> Vec<CollabSnapshot> {
    let store = self.collab_db.read_txn();
    store.get_snapshots(self.uid, database_id)
  }

  pub fn restore_database_from_snapshot(
    &self,
    database_id: &str,
    snapshot: CollabSnapshot,
  ) -> Result<Database, DatabaseError> {
    let collab = self.collab_for_database(database_id, CollabRawData::default());
    let update = Update::decode_v1(&snapshot.data)?;
    collab.lock().with_transact_mut(|txn| {
      txn.apply_update(update);
    });

    let context = DatabaseContext {
      collab,
      block: self.block.clone(),
      collab_builder: self.collab_service.clone(),
    };
    Database::get_or_create(database_id, context)
  }

  /// Delete the view from the database with the given view id.
  /// If the view is the inline view, the database will be deleted too.
  pub async fn delete_view(&self, database_id: &str, view_id: &str) {
    if let Some(database) = self.get_database(database_id).await {
      database.lock().delete_view(view_id);
      if database.lock().is_inline_view(view_id) {
        // Delete the database if the view is the inline view.
        self.delete_database(database_id);
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
      let data = database.lock().duplicate_database();
      Ok(data)
    } else {
      Err(DatabaseError::DatabaseNotExist)
    }
  }

  /// Create a new [Collab] instance for given database id.
  fn collab_for_database(
    &self,
    database_id: &str,
    collab_raw_data: CollabRawData,
  ) -> Arc<MutexCollab> {
    self.collab_service.build_collab_with_config(
      self.uid,
      database_id,
      "database",
      self.collab_db.clone(),
      collab_raw_data,
      &self.config,
    )
  }

  fn database_array(&self) -> DatabaseArray {
    get_database_array_ref(&self.collab.lock())
  }
}

fn get_database_array_ref(collab: &Collab) -> DatabaseArray {
  let array = {
    let txn = collab.transact();
    collab.get_array_with_txn(&txn, vec![DATABASES])
  };

  let databases = match array {
    None => collab.with_transact_mut(|txn| {
      collab.create_array_with_txn::<MapPrelim<lib0Any>>(txn, DATABASES, vec![])
    }),
    Some(array) => array,
  };
  DatabaseArray::new(databases)
}
