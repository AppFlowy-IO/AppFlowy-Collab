use collab::core::collab::DataSource;
use collab::core::collab_plugin::CollabPersistence;
use collab::preclude::Collab;
use collab_plugins::local_storage::kv::doc::CollabKVAction;
use collab_plugins::local_storage::kv::KVTransactionDB;
use collab_plugins::CollabKVDB;
use std::sync::Weak;
use tracing::{error, warn};

pub struct KVDBCollabPersistenceImpl {
  pub db: Weak<CollabKVDB>,
  pub uid: i64,
}

impl KVDBCollabPersistenceImpl {
  pub fn new(db: Weak<CollabKVDB>, uid: i64) -> Self {
    Self { db, uid }
  }

  pub fn into_data_source(self) -> DataSource {
    DataSource::Disk(Some(Box::new(self)))
  }
}

impl From<KVDBCollabPersistenceImpl> for DataSource {
  fn from(persistence: KVDBCollabPersistenceImpl) -> Self {
    persistence.into_data_source()
  }
}

impl CollabPersistence for KVDBCollabPersistenceImpl {
  fn load_collab(&self, collab: &mut Collab) {
    if let Some(collab_db) = self.db.upgrade() {
      let object_id = collab.object_id().to_string();
      let rocksdb_read = collab_db.read_txn();

      if rocksdb_read.is_exist(self.uid, &object_id) {
        let mut txn = collab.transact_mut();
        if let Err(err) = rocksdb_read.load_doc_with_txn(self.uid, &object_id, &mut txn) {
          error!("🔴 load doc:{} failed: {}", object_id, err);
        }
        drop(rocksdb_read);
        txn.commit();
        drop(txn);
      }
    } else {
      warn!("collab_db is dropped");
    }
  }
}
