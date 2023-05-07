use std::collections::HashMap;
use std::sync::Arc;

use collab::preclude::updates::decoder::Decode;
use collab::preclude::{lib0Any, ArrayRefWrapper, Collab, CollabBuilder, MapPrelim, Update};
use collab_persistence::doc::YrsDocAction;
use collab_persistence::kv::rocks_kv::RocksCollabDB;
use collab_persistence::snapshot::{CollabSnapshot, SnapshotAction};
use parking_lot::RwLock;
use collab_plugins::disk_plugin::rocks_disk::{Config, RocksDiskPlugin};

use crate::blocks::Block;
use crate::database::{Database, DatabaseContext, DatabaseData};
use crate::error::DatabaseError;
use crate::user::db_record::{DatabaseArray, DatabaseRecord};
use crate::user::relation::{DatabaseRelation, RowRelationMap};
use crate::views::{CreateDatabaseParams, CreateViewParams};

/// A [UserDatabase] represents a user's database.
pub struct UserDatabase {
  uid: i64,
  db: Arc<RocksCollabDB>,
  #[allow(dead_code)]
  collab: Collab,
  /// It used to keep track of the blocks. Each block contains a list of [Row]s
  /// A database rows will be stored in multiple blocks.
  block: Block,
  /// It used to keep track of the database records.
  database_records: DatabaseArray,
  /// It used to keep track of the database relations.
  database_relation: DatabaseRelation,
  /// In memory database handlers.
  /// The key is the database id. The handler will be added when the database is opened or created.
  /// and the handler will be removed when the database is deleted or closed.
  open_handlers: RwLock<HashMap<String, Arc<Database>>>,
  config: Config,
}

const DATABASES: &str = "databases";

impl UserDatabase {
  pub fn new(uid: i64, db: Arc<RocksCollabDB>, config: Config) -> Self {
    tracing::trace!("Init user database: {}", uid);
    // user database
    let disk_plugin = RocksDiskPlugin::new_with_config(uid, db.clone(), config.clone()).unwrap();
    let collab = CollabBuilder::new(uid, format!("{}_user_database", uid))
      .with_plugin(disk_plugin)
      .build();
    collab.initial();
    let databases = create_user_database_if_not_exist(&collab);
    let database_records = DatabaseArray::new(databases);
    let database_relation = DatabaseRelation::new(create_relations_collab(uid, db.clone()));
    let block = Block::new(uid, db.clone());

    Self {
      uid,
      db,
      collab,
      block,
      database_records,
      open_handlers: Default::default(),
      database_relation,
      config,
    }
  }

  /// Get the database with the given database id.
  /// Return None if the database does not exist.
  pub fn get_database(&self, database_id: &str) -> Option<Arc<Database>> {
    if !self.database_records.contains(database_id) {
      return None;
    }
    let database = self.open_handlers.read().get(database_id).cloned();
    match database {
      None => {
        let blocks = self.block.clone();
        let collab = self.collab_for_database(database_id);
        collab.initial();
        let context = DatabaseContext {
          collab,
          block: blocks,
        };
        let database = Arc::new(Database::get_or_create(database_id, context).ok()?);
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
  pub fn get_database_with_view_id(&self, view_id: &str) -> Option<Arc<Database>> {
    let database_id = self.get_database_id_with_view_id(view_id)?;
    self.get_database(&database_id)
  }

  /// Return the database id with the given view id.
  pub fn get_database_id_with_view_id(&self, view_id: &str) -> Option<String> {
    self
      .database_records
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
  ) -> Result<Arc<Database>, DatabaseError> {
    debug_assert!(!params.database_id.is_empty());
    debug_assert!(!params.view_id.is_empty());

    // Create a [Collab] for the given database id.
    let collab = self.collab_for_database(&params.database_id);
    let blocks = self.block.clone();
    let context = DatabaseContext {
      collab,
      block: blocks,
    };
    context.collab.initial();

    // Add a new database record.
    self
      .database_records
      .add_database(&params.database_id, &params.view_id, &params.name);
    let database_id = params.database_id.clone();
    let database = Arc::new(Database::create_with_inline_view(params, context)?);
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
  ) -> Result<Arc<Database>, DatabaseError> {
    let DatabaseData { view, fields, rows } = data;
    let params = CreateDatabaseParams::from_view(view, fields, rows);
    let database = self.create_database(params)?;
    Ok(database)
  }

  /// Create reference view that shares the same data with the inline view's database
  /// If the inline view is deleted, the reference view will be deleted too.
  pub fn create_database_view(&self, params: CreateViewParams) {
    if let Some(database) = self.get_database(&params.database_id) {
      self
        .database_records
        .update_database(&params.database_id, |record| {
          record.views.insert(params.view_id.clone());
        });
      database.create_linked_view(params);
    }
  }

  /// Delete the database with the given database id.
  pub fn delete_database(&self, database_id: &str) {
    self.database_records.delete_database(database_id);
    let _ = self.db.with_write_txn(|w_db_txn| {
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
    self.database_records.get_all_databases()
  }

  pub fn get_database_snapshots(&self, database_id: &str) -> Vec<CollabSnapshot> {
    let store = self.db.read_txn();
    store.get_snapshots(self.uid, database_id)
  }

  pub fn restore_database_from_snapshot(
    &self,
    database_id: &str,
    snapshot: CollabSnapshot,
  ) -> Result<Database, DatabaseError> {
    let collab = self.collab_for_database(database_id);
    let update = Update::decode_v1(&snapshot.data)?;
    collab.with_transact_mut(|txn| {
      txn.apply_update(update);
    });

    let context = DatabaseContext {
      collab,
      block: self.block.clone(),
    };
    Database::get_or_create(database_id, context)
  }

  /// Delete the view from the database with the given view id.
  /// If the view is the inline view, the database will be deleted too.
  pub fn delete_view(&self, database_id: &str, view_id: &str) {
    if let Some(database) = self.get_database(database_id) {
      if database.is_inline_view(view_id) {
        self.delete_database(database_id);
      }
      database.delete_view(view_id);
    }
  }

  /// Duplicate the database that contains the view.
  pub fn duplicate_database(&self, view_id: &str) -> Result<Arc<Database>, DatabaseError> {
    let DatabaseData { view, fields, rows } = self.get_database_duplicated_data(view_id)?;
    let params = CreateDatabaseParams::from_view(view, fields, rows);
    let database = self.create_database(params)?;
    Ok(database)
  }

  /// Duplicate the database with the given view id.
  pub fn get_database_duplicated_data(&self, view_id: &str) -> Result<DatabaseData, DatabaseError> {
    if let Some(database) = self.get_database_with_view_id(view_id) {
      let data = database.duplicate_database();
      Ok(data)
    } else {
      Err(DatabaseError::DatabaseNotExist)
    }
  }

  pub fn relations(&self) -> &RowRelationMap {
    self.database_relation.row_relations()
  }

  /// Create a new [Collab] instance for given database id.
  fn collab_for_database(&self, database_id: &str) -> Collab {
    let disk_plugin =
      RocksDiskPlugin::new_with_config(self.uid, self.db.clone(), self.config.clone()).unwrap();
    CollabBuilder::new(self.uid, database_id)
      .with_plugin(disk_plugin)
      .build()
  }
}

fn create_user_database_if_not_exist(collab: &Collab) -> ArrayRefWrapper {
  let array = {
    let txn = collab.transact();
    collab.get_array_with_txn(&txn, vec![DATABASES])
  };

  match array {
    None => collab.with_transact_mut(|txn| {
      collab.create_array_with_txn::<MapPrelim<lib0Any>>(txn, DATABASES, vec![])
    }),
    Some(array) => array,
  }
}

fn create_relations_collab(uid: i64, db: Arc<RocksCollabDB>) -> Collab {
  let disk_plugin = RocksDiskPlugin::new(uid, db).unwrap();
  let object_id = format!("{}_db_relations", uid);
  let collab = CollabBuilder::new(uid, object_id)
    .with_plugin(disk_plugin)
    .build();
  collab.initial();
  collab
}
