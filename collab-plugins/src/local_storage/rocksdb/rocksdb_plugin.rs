use std::ops::Deref;
use std::sync::atomic::AtomicU32;
use std::sync::atomic::Ordering::SeqCst;
use std::sync::{Arc, Weak};

use crate::local_storage::kv::doc::CollabKVAction;
use crate::local_storage::kv::KVTransactionDB;
use crate::local_storage::CollabPersistenceConfig;
use crate::CollabKVDB;

use collab::entity::EncodedCollab;
use collab::preclude::{Collab, CollabPlugin};
use collab_entity::CollabType;
use tracing::error;

use yrs::TransactionMut;

pub trait RocksdbBackup: Send + Sync {
  fn save_doc(&self, uid: i64, object_id: &str, data: EncodedCollab) -> Result<(), anyhow::Error>;
  fn get_doc(&self, uid: i64, object_id: &str) -> Result<EncodedCollab, anyhow::Error>;
}

#[derive(Clone)]
pub struct RocksdbDiskPlugin {
  uid: i64,
  #[allow(dead_code)]
  object_id: String,
  collab_type: CollabType,
  collab_db: Weak<CollabKVDB>,
  update_count: Arc<AtomicU32>,
  #[allow(dead_code)]
  config: CollabPersistenceConfig,
}

impl Deref for RocksdbDiskPlugin {
  type Target = Weak<CollabKVDB>;

  fn deref(&self) -> &Self::Target {
    &self.collab_db
  }
}

impl RocksdbDiskPlugin {
  pub fn new_with_config(
    uid: i64,
    object_id: String,
    collab_type: CollabType,
    collab_db: Weak<CollabKVDB>,
    config: CollabPersistenceConfig,
  ) -> Self {
    let update_count = Arc::new(AtomicU32::new(0));
    Self {
      object_id,
      collab_type,
      collab_db,
      uid,
      update_count,
      config,
    }
  }

  pub fn new(
    uid: i64,
    object_id: String,
    collab_type: CollabType,
    collab_db: Weak<CollabKVDB>,
  ) -> Self {
    Self::new_with_config(
      uid,
      object_id,
      collab_type,
      collab_db,
      CollabPersistenceConfig::default(),
    )
  }

  fn increase_count(&self) {
    let _update_count = self.update_count.fetch_add(1, SeqCst);
  }
}

impl CollabPlugin for RocksdbDiskPlugin {
  fn did_init(&self, collab: &Collab, object_id: &str) {
    if let Some(collab_db) = self.collab_db.upgrade() {
      let rocksdb_read = collab_db.read_txn();
      if !rocksdb_read.is_exist(self.uid, object_id) {
        let txn = collab.transact();
        if let Err(err) = collab_db.with_write_txn(|w_db_txn| {
          w_db_txn.create_new_doc(self.uid, &object_id, &txn)?;
          tracing::trace!("Created new doc {}", object_id);
          Ok(())
        }) {
          error!("create doc for {:?} failed: {}", object_id, err);
        }
      }
    }
  }

  fn receive_update(&self, object_id: &str, _txn: &TransactionMut, update: &[u8]) {
    // Only push update if the doc is loaded
    if let Some(db) = self.collab_db.upgrade() {
      self.increase_count();
      //Acquire a write transaction to ensure consistency
      let result = db.with_write_txn(|w_db_txn| {
        let _ = w_db_txn.push_update(self.uid, object_id, update)?;
        #[cfg(not(feature = "verbose_log"))]
        tracing::trace!(
          "Collab {} {} persisting update",
          object_id,
          self.collab_type
        );
        #[cfg(feature = "verbose_log")]
        {
          use yrs::updates::decoder::Decode;
          let update = yrs::Update::decode_v1(update).unwrap();
          tracing::trace!(
            "Collab {} {} persisting update: {:#?}",
            object_id,
            self.collab_type,
            update
          );
        }
        Ok(())
      });

      if let Err(e) = result {
        error!(
          "{}:{} save update failed: {:?}",
          object_id, self.collab_type, e
        );
      }
    } else {
      tracing::warn!("collab_db is dropped");
    };
  }
}
