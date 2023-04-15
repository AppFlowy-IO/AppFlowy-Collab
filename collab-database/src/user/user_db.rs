use std::collections::HashMap;
use std::sync::Arc;

use collab::plugin_impl::disk::CollabDiskPlugin;
use collab::plugin_impl::snapshot::CollabSnapshotPlugin;
use collab::preclude::updates::decoder::Decode;
use collab::preclude::{lib0Any, Collab, CollabBuilder, MapPrelim, Update};
use collab_persistence::snapshot::CollabSnapshot;
use collab_persistence::CollabKV;
use parking_lot::RwLock;

use crate::block::Blocks;
use crate::database::{Database, DatabaseContext, DuplicatedDatabase};
use crate::error::DatabaseError;
use crate::user::db_record::{DatabaseArray, DatabaseRecord};
use crate::user::relation::{DatabaseRelation, RowRelationMap};
use crate::views::{CreateDatabaseParams, CreateViewParams};

pub struct UserDatabase {
  uid: i64,
  db: Arc<CollabKV>,
  #[allow(dead_code)]
  collab: Collab,
  blocks: Blocks,
  database_records: DatabaseArray,
  database_relation: DatabaseRelation,
  open_handlers: RwLock<HashMap<String, Arc<Database>>>,
}

const DATABASES: &str = "databases";

impl UserDatabase {
  pub fn new(uid: i64, db: Arc<CollabKV>) -> Self {
    tracing::trace!("Init user database: {}", uid);
    let disk_plugin = CollabDiskPlugin::new(uid, db.clone()).unwrap();
    let snapshot_plugin = CollabSnapshotPlugin::new(uid, db.clone(), 5).unwrap();
    let collab = CollabBuilder::new(uid, "user_database")
      .with_plugin(disk_plugin)
      .with_plugin(snapshot_plugin)
      .build();
    collab.initial();
    let databases = collab.with_transact_mut(|txn| {
      // { DATABASES: {:} }
      let databases = collab
        .get_array_with_txn(txn, vec![DATABASES])
        .unwrap_or_else(|| {
          collab.create_array_with_txn::<MapPrelim<lib0Any>>(txn, DATABASES, vec![])
        });

      databases
    });
    let database_vec = DatabaseArray::new(databases);
    let database_relation = DatabaseRelation::new(create_relations_collab(uid, db.clone()));
    let blocks = Blocks::new(uid, db.clone());
    Self {
      uid,
      db,
      collab,
      blocks,
      database_records: database_vec,
      open_handlers: Default::default(),
      database_relation,
    }
  }

  pub fn get_database(&self, database_id: &str) -> Option<Arc<Database>> {
    if !self.database_records.contains(database_id) {
      return None;
    }
    let database = self.open_handlers.read().get(database_id).cloned();
    match database {
      None => {
        let context = DatabaseContext {
          collab: self.collab_for_database(database_id),
          blocks: self.blocks.clone(),
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
  pub fn get_database_with_view_id(&self, view_id: &str) -> Option<Arc<Database>> {
    let database_id = self.get_database_id_with_view_id(view_id)?;
    self.get_database(&database_id)
  }

  pub fn get_database_id_with_view_id(&self, view_id: &str) -> Option<String> {
    self
      .database_records
      .get_database_record_with_view_id(view_id)
      .map(|record| record.database_id)
  }

  /// Create database with inline view
  pub fn create_database(
    &self,
    params: CreateDatabaseParams,
  ) -> Result<Arc<Database>, DatabaseError> {
    let context = DatabaseContext {
      collab: self.collab_for_database(&params.database_id),
      blocks: self.blocks.clone(),
    };
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

  pub fn create_database_with_duplicated_data(
    &self,
    data: DuplicatedDatabase,
  ) -> Result<Arc<Database>, DatabaseError> {
    let DuplicatedDatabase { view, fields, rows } = data;
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

  pub fn delete_database(&self, database_id: &str) {
    self.database_records.delete_database(database_id);
    match self.db.doc(self.uid).delete_doc(database_id) {
      Ok(_) => {},
      Err(err) => tracing::error!("Delete database failed: {}", err),
    }
    match self.db.snapshot(self.uid).delete_snapshot(database_id) {
      Ok(_) => {},
      Err(err) => tracing::error!("Delete snapshot failed: {}", err),
    }
  }

  pub fn get_all_databases(&self) -> Vec<DatabaseRecord> {
    self.database_records.get_all_databases()
  }

  pub fn get_database_snapshots(&self, database_id: &str) -> Vec<CollabSnapshot> {
    self.db.snapshot(self.uid).get_snapshots(database_id)
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
      blocks: self.blocks.clone(),
    };
    Database::get_or_create(database_id, context)
  }

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
    let DuplicatedDatabase { view, fields, rows } = self.make_duplicate_database_data(view_id)?;
    let params = CreateDatabaseParams::from_view(view, fields, rows);
    let database = self.create_database(params)?;
    Ok(database)
  }

  /// Duplicate the view in the database.
  pub fn make_duplicate_database_data(
    &self,
    view_id: &str,
  ) -> Result<DuplicatedDatabase, DatabaseError> {
    if let Some(database) = self.get_database_with_view_id(view_id) {
      let data = database.duplicate_database_data();
      Ok(data)
    } else {
      Err(DatabaseError::DatabaseNotExist)
    }
  }

  pub fn relations(&self) -> &RowRelationMap {
    self.database_relation.row_relations()
  }

  fn collab_for_database(&self, database_id: &str) -> Collab {
    let disk_plugin = CollabDiskPlugin::new(self.uid, self.db.clone()).unwrap();
    let snapshot_plugin = CollabSnapshotPlugin::new(self.uid, self.db.clone(), 6).unwrap();
    CollabBuilder::new(self.uid, database_id)
      .with_plugin(disk_plugin)
      .with_plugin(snapshot_plugin)
      .build()
  }
}

fn create_relations_collab(uid: i64, db: Arc<CollabKV>) -> Collab {
  let disk_plugin = CollabDiskPlugin::new(uid, db).unwrap();
  let object_id = format!("{}_db_relations", uid);
  let collab = CollabBuilder::new(uid, object_id)
    .with_plugin(disk_plugin)
    .build();
  collab.initial();
  collab
}
