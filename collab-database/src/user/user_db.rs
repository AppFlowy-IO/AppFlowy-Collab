use crate::database::{Database, DatabaseContext};
use crate::error::DatabaseError;
use crate::user::user_db_record::{DatabaseArray, DatabaseRecord};
use crate::views::CreateViewParams;
use anyhow::Context;
use collab::plugin_impl::disk::CollabDiskPlugin;
use collab::plugin_impl::snapshot::CollabSnapshotPlugin;
use collab::preclude::updates::decoder::Decode;
use collab::preclude::{
  lib0Any, Array, ArrayRefWrapper, Collab, CollabBuilder, MapPrelim, MapRefWrapper, TransactionMut,
  Update,
};
use collab_persistence::snapshot::CollabSnapshot;
use collab_persistence::CollabKV;
use std::sync::Arc;

pub struct UserDatabase {
  uid: i64,
  db: Arc<CollabKV>,
  collab: Collab,
  databases: DatabaseArray,
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
      let databases = collab
        .get_array_with_txn(txn, vec![DATABASES])
        .unwrap_or_else(|| {
          collab.create_array_with_txn::<MapPrelim<lib0Any>>(txn, DATABASES, vec![])
        });

      databases
    });

    let databases = DatabaseArray::new(databases);
    Self {
      uid,
      db,
      collab,
      databases,
    }
  }

  pub fn get_database(&self, database_id: &str) -> Database {
    let context = DatabaseContext {
      collab: self.collab_for_database(database_id),
    };
    Database::create(database_id, context).unwrap()
  }

  pub fn create_database(
    &self,
    database_id: &str,
    params: CreateViewParams,
  ) -> Result<Database, DatabaseError> {
    let context = DatabaseContext {
      collab: self.collab_for_database(database_id),
    };
    self.databases.add_database(database_id, &params.name);
    let database = Database::create_with_view(database_id, params, context)?;
    Ok(database)
  }

  pub fn delete_database(&self, database_id: &str) {
    self.databases.delete_database(database_id);
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
    self.databases.get_all_databases()
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

  fn collab_for_database(&self, database_id: &str) -> Collab {
    let disk_plugin = CollabDiskPlugin::new(self.uid, self.db.clone()).unwrap();
    let snapshot_plugin = CollabSnapshotPlugin::new(self.uid, self.db.clone(), 5).unwrap();
    CollabBuilder::new(self.uid, database_id)
      .with_plugin(disk_plugin)
      .with_plugin(snapshot_plugin)
      .build()
  }
}
