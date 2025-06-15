use crate::CollabKVDB;
use crate::local_storage::CollabPersistenceConfig;
use crate::local_storage::kv::KVTransactionDB;
use crate::local_storage::kv::doc::CollabKVAction;

use std::ops::Deref;
use std::sync::atomic::Ordering::SeqCst;
use std::sync::atomic::{AtomicBool, AtomicU32};
use std::sync::{Arc, Weak};

use collab::entity::EncodedCollab;
use collab::preclude::{Collab, CollabPlugin};
use collab_entity::CollabType;
use tracing::{error, info, warn};

use collab::core::collab_plugin::CollabPluginType;
use yrs::TransactionMut;

pub trait RocksdbBackup: Send + Sync {
  fn save_doc(&self, uid: i64, object_id: &str, data: EncodedCollab) -> Result<(), anyhow::Error>;
  fn get_doc(&self, uid: i64, object_id: &str) -> Result<EncodedCollab, anyhow::Error>;
}

#[derive(Clone)]
pub struct RocksdbDiskPlugin {
  uid: i64,
  #[allow(dead_code)]
  workspace_id: String,
  object_id: String,
  collab_type: CollabType,
  collab_db: Weak<CollabKVDB>,
  did_init: Arc<AtomicBool>,
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
    workspace_id: String,
    object_id: String,
    collab_type: CollabType,
    collab_db: Weak<CollabKVDB>,
    config: CollabPersistenceConfig,
  ) -> Self {
    let update_count = Arc::new(AtomicU32::new(0));
    let did_init = Arc::new(AtomicBool::new(false));
    Self {
      workspace_id,
      object_id,
      collab_type,
      collab_db,
      uid,
      did_init,
      update_count,
      config,
    }
  }

  pub fn new(
    uid: i64,
    workspace_id: String,
    object_id: String,
    collab_type: CollabType,
    collab_db: Weak<CollabKVDB>,
  ) -> Self {
    Self::new_with_config(
      uid,
      workspace_id,
      object_id,
      collab_type,
      collab_db,
      CollabPersistenceConfig::default(),
    )
  }

  fn increase_count(&self) {
    let _update_count = self.update_count.fetch_add(1, SeqCst);
  }

  fn write_to_disk(&self, collab: &Collab) {
    if let Some(collab_db) = self.collab_db.upgrade() {
      let rocksdb_read = collab_db.read_txn();
      if !rocksdb_read.is_exist(self.uid, &self.workspace_id, &self.object_id) {
        match self.collab_type.validate_require_data(collab) {
          Ok(_) => {
            let txn = collab.transact();
            if let Err(err) = collab_db.with_write_txn(|w_db_txn| {
              w_db_txn.create_new_doc(self.uid, &self.workspace_id, &self.object_id, &txn)?;
              info!(
                "[Rocksdb Plugin]: created new doc {}, collab_type:{}",
                self.object_id, self.collab_type
              );
              Ok(())
            }) {
              error!(
                "[Rocksdb Plugin]: create doc:{} failed: {}",
                self.object_id, err
              );
            }
          },
          Err(err) => {
            warn!(
              "[Rocksdb Plugin]: validate collab:{}, collab_type:{}, failed: {}",
              self.object_id, self.collab_type, err
            );
          },
        }
      }
    }
  }
}

impl CollabPlugin for RocksdbDiskPlugin {
  fn did_init(&self, collab: &Collab, _object_id: &str) {
    self.did_init.store(true, SeqCst);
    self.write_to_disk(collab);
  }

  fn receive_update(&self, object_id: &str, _txn: &TransactionMut, update: &[u8]) {
    // Only push update if the doc is loaded
    if !self.did_init.load(SeqCst) {
      return;
    }
    if let Some(db) = self.collab_db.upgrade() {
      self.increase_count();
      //Acquire a write transaction to ensure consistency
      let result = db.with_write_txn(|w_db_txn| {
        let _ = w_db_txn.push_update(self.uid, self.workspace_id.as_str(), object_id, update)?;
        use yrs::updates::decoder::Decode;
        tracing::trace!(
          "[Rocksdb Plugin]: Collab {} {} persisting update: {:#?}",
          object_id,
          self.collab_type,
          yrs::Update::decode_v1(update).unwrap()
        );
        Ok(())
      });

      if let Err(err) = result {
        error!(
          "[Rocksdb Plugin]: {}:{} save update failed: {:?}",
          object_id, self.collab_type, err
        );
      }
    } else {
      tracing::warn!("[Rocksdb Plugin]: collab_db is dropped");
    };
  }

  fn plugin_type(&self) -> CollabPluginType {
    CollabPluginType::Other("RocksdbDiskPlugin".to_string())
  }
}
