use crate::CollabKVDB;
use crate::local_storage::kv::KVTransactionDB;
use crate::local_storage::kv::doc::CollabKVAction;
use anyhow::anyhow;
use collab::core::collab::DataSource;
use collab::core::collab_plugin::CollabPersistence;
use collab::error::CollabError;
use collab::preclude::Collab;
use std::sync::Weak;
use tracing::error;

pub struct KVDBCollabPersistenceImpl {
  pub db: Weak<CollabKVDB>,
  pub uid: i64,
  pub workspace_id: String,
}

impl KVDBCollabPersistenceImpl {
  pub fn new(db: Weak<CollabKVDB>, uid: i64, workspace_id: String) -> Self {
    Self {
      db,
      uid,
      workspace_id,
    }
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
  fn load_collab_from_disk(&self, collab: &mut Collab) -> Result<(), CollabError> {
    let collab_db = self
      .db
      .upgrade()
      .ok_or_else(|| CollabError::Internal(anyhow!("collab_db is dropped")))?;
    let object_id = collab.object_id().to_string();
    let rocksdb_read = collab_db.read_txn();

    if rocksdb_read.is_exist(self.uid, &self.workspace_id, &object_id) {
      let mut txn = collab.transact_mut();
      if let Err(err) =
        rocksdb_read.load_doc_with_txn(self.uid, self.workspace_id.as_str(), &object_id, &mut txn)
      {
        error!("🔴 load doc:{} failed: {}", object_id, err);
      }
      drop(rocksdb_read);
      txn.commit();
      drop(txn);
    }
    Ok(())
  }
}
