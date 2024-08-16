use std::ops::Deref;
use std::sync::atomic::Ordering::SeqCst;
use std::sync::atomic::{AtomicBool, AtomicU32};
use std::sync::{Arc, Weak};

use crate::local_storage::kv::doc::CollabKVAction;
use crate::local_storage::kv::snapshot::SnapshotPersistence;
use crate::local_storage::kv::KVTransactionDB;
use crate::local_storage::rocksdb::snapshot_plugin::CollabSnapshot;
use crate::local_storage::CollabPersistenceConfig;
use crate::CollabKVDB;
use collab::core::collab::make_yrs_doc;

use collab::entity::EncodedCollab;
use collab::preclude::{Collab, CollabPlugin};
use collab_entity::CollabType;
use tracing::{debug, error};
use yrs::updates::encoder::Encode;
use yrs::{ReadTxn, StateVector, Transact, TransactionMut};

pub trait RocksdbBackup: Send + Sync {
  fn save_doc(&self, uid: i64, object_id: &str, data: EncodedCollab) -> Result<(), anyhow::Error>;
  fn get_doc(&self, uid: i64, object_id: &str) -> Result<EncodedCollab, anyhow::Error>;
}

#[derive(Clone)]
pub struct RocksdbDiskPlugin {
  uid: i64,
  object_id: String,
  collab_type: CollabType,
  collab_db: Weak<CollabKVDB>,
  did_init: Arc<AtomicBool>,
  update_count: Arc<AtomicU32>,
  config: CollabPersistenceConfig,
  snapshot: Option<CollabSnapshot>,
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
    snapshot_persistence: Option<Arc<dyn SnapshotPersistence>>,
  ) -> Self {
    let update_count = Arc::new(AtomicU32::new(0));
    let did_init = Arc::new(AtomicBool::new(false));

    let mut snapshot = None;
    if config.enable_snapshot {
      snapshot = snapshot_persistence.map(CollabSnapshot::new);
    }

    Self {
      object_id,
      collab_type,
      collab_db,
      uid,
      did_init,
      update_count,
      config,
      snapshot,
    }
  }

  pub fn new(
    uid: i64,
    object_id: String,
    collab_type: CollabType,
    collab_db: Weak<CollabKVDB>,
    snapshot_persistence: Option<Arc<dyn SnapshotPersistence>>,
  ) -> Self {
    Self::new_with_config(
      uid,
      object_id,
      collab_type,
      collab_db,
      CollabPersistenceConfig::default(),
      snapshot_persistence,
    )
  }

  fn increase_count(&self) {
    let update_count = self.update_count.fetch_add(1, SeqCst);
    self.create_snapshot_if_need(update_count);
  }

  fn create_snapshot_if_need(&self, update_count: u32) {
    if update_count != 0 && update_count % self.config.snapshot_per_update == 0 {
      if let Some(snapshot) = &self.snapshot {
        if snapshot.should_create_snapshot() {
          debug!(
            "create snapshot for {}, update_count:{}, snapshot_per_update:{}",
            self.object_id, update_count, self.config.snapshot_per_update
          );
          snapshot.create_snapshot(
            self.collab_db.clone(),
            self.uid,
            &self.object_id,
            &self.collab_type,
          );
        }
      }
      self.update_count.store(0, SeqCst);
    }
  }

  fn flush_doc(&self, db: &Arc<CollabKVDB>, object_id: &str) {
    let _ = db.with_write_txn(|w_db_txn| {
      let doc = make_yrs_doc(false);
      w_db_txn.load_doc_with_txn(self.uid, object_id, &mut doc.transact_mut())?;
      if let Ok(read_txn) = doc.try_transact() {
        let doc_state = read_txn.encode_state_as_update_v1(&StateVector::default());
        let state_vector = read_txn.state_vector().encode_v1();
        let encoded = EncodedCollab::new_v1(state_vector, doc_state);

        w_db_txn.flush_doc(
          self.uid,
          object_id,
          encoded.state_vector.to_vec(),
          encoded.doc_state.to_vec(),
        )?;
      }

      Ok(())
    });
  }
}

impl CollabPlugin for RocksdbDiskPlugin {
  fn did_init(&self, collab: &Collab, object_id: &str, _last_sync_at: i64) {
    self.did_init.store(true, SeqCst);

    if let Some(collab_db) = self.collab_db.upgrade() {
      let rocksdb_read = collab_db.read_txn();
      if !rocksdb_read.is_exist(self.uid, object_id) {
        let txn = collab.transact();
        if let Err(err) = collab_db.with_write_txn(|w_db_txn| {
          w_db_txn.create_new_doc(self.uid, &object_id, &txn)?;
          Ok(())
        }) {
          error!("create doc for {:?} failed: {}", object_id, err);
        }
      }
    }
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
        let _ = w_db_txn.push_update(self.uid, object_id, update)?;
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

  fn write_to_disk(&self, object_id: &str) {
    if let Some(db) = self.collab_db.upgrade() {
      self.flush_doc(&db, object_id);
    }
  }
}
