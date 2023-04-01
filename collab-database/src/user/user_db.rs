use crate::database::{Database, DatabaseContext};
use crate::error::DatabaseError;
use crate::user::user_db_record::{DatabaseArray, DatabaseRecord};
use crate::views::CreateViewParams;

use collab::plugin_impl::disk::CollabDiskPlugin;
use collab::plugin_impl::snapshot::CollabSnapshotPlugin;
use collab::preclude::updates::decoder::Decode;
use collab::preclude::{lib0Any, Collab, CollabBuilder, MapPrelim, Update};
use collab_persistence::snapshot::CollabSnapshot;
use collab_persistence::CollabKV;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

pub struct UserDatabase {
  uid: i64,
  db: Arc<CollabKV>,
  #[allow(dead_code)]
  collab: Collab,
  database_vec: DatabaseArray,
  open_handlers: RwLock<HashMap<String, Arc<Database>>>,
}

const DATABASES: &str = "databases";

impl UserDatabase {
  pub fn new(uid: i64, db: Arc<CollabKV>) -> Self {
    let disk_plugin = CollabDiskPlugin::new(uid, db.clone()).unwrap();
    let snapshot_plugin = CollabSnapshotPlugin::new(uid, db.clone(), 5).unwrap();
    let collab = CollabBuilder::new(uid, "user_database")
      .with_plugin(disk_plugin)
      .with_plugin(snapshot_plugin)
      .build();
    collab.initial();
    let databases = collab.with_transact_mut(|txn| {
      // { DATABASES: {:} }
      collab
        .get_array_with_txn(txn, vec![DATABASES])
        .unwrap_or_else(|| {
          collab.create_array_with_txn::<MapPrelim<lib0Any>>(txn, DATABASES, vec![])
        })
    });

    let database_vec = DatabaseArray::new(databases);
    Self {
      uid,
      db,
      collab,
      database_vec,
      open_handlers: Default::default(),
    }
  }

  pub fn get_database(&self, database_id: &str) -> Option<Arc<Database>> {
    if !self.database_vec.contains(database_id) {
      return None;
    }
    let database = self.open_handlers.read().get(database_id).cloned();
    match database {
      None => {
        let context = DatabaseContext {
          collab: self.collab_for_database(database_id),
        };
        let database = Arc::new(Database::create(database_id, context).ok()?);
        self
          .open_handlers
          .write()
          .insert(database_id.to_string(), database.clone());
        Some(database)
      },
      Some(database) => Some(database),
    }
  }

  pub fn create_database(
    &self,
    database_id: &str,
    params: CreateViewParams,
  ) -> Result<Arc<Database>, DatabaseError> {
    let context = DatabaseContext {
      collab: self.collab_for_database(database_id),
    };
    self.database_vec.add_database(database_id, &params.name);
    let database = Arc::new(Database::create_with_view(database_id, params, context)?);
    self
      .open_handlers
      .write()
      .insert(database_id.to_string(), database.clone());
    Ok(database)
  }

  pub fn delete_database(&self, database_id: &str) {
    self.database_vec.delete_database(database_id);
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
    self.database_vec.get_all_databases()
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

    let context = DatabaseContext { collab };
    Database::create(database_id, context)
  }

  pub fn delete_view(&self, database_id: &str, view_id: &str) {
    if let Some(database) = self.get_database(database_id) {
      if database.is_inline_view(view_id) {
        self.delete_database(database_id);
      }
      database.delete_view(view_id);
    }
  }

  pub fn duplicate_view(&self, database_id: &str, _view_id: &str) {
    if let Some(_database) = self.get_database(database_id) {
      // if database.is_inline_view(view_id) {
      // } else {
      // }
    }
  }

  fn collab_for_database(&self, database_id: &str) -> Collab {
    let disk_plugin = CollabDiskPlugin::new(self.uid, self.db.clone()).unwrap();
    let snapshot_plugin = CollabSnapshotPlugin::new(self.uid, self.db.clone(), 5).unwrap();
    CollabBuilder::new(self.uid, database_id)
      .with_plugin(disk_plugin)
      .with_plugin(snapshot_plugin)
      .build()
  }
}
