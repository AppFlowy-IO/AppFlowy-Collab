use std::ops::Deref;
use std::sync::atomic::Ordering::SeqCst;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Weak};

use collab::core::collab::make_yrs_doc;
use collab::core::collab_plugin::EncodedCollabV1;
use collab::core::origin::CollabOrigin;
use collab::preclude::CollabPlugin;
use collab::sync_protocol::awareness::Awareness;
use collab_persistence::doc::YrsDocAction;
use collab_persistence::kv::rocks_kv::RocksCollabDB;
use tracing::{error, instrument};
use yrs::updates::encoder::Encode;
use yrs::{Doc, ReadTxn, StateVector, Transact, TransactionMut};

use crate::local_storage::CollabPersistenceConfig;

pub trait RocksdbBackup: Send + Sync {
  fn save_doc(&self, uid: i64, object_id: &str, data: EncodedCollabV1)
    -> Result<(), anyhow::Error>;
  fn get_doc(&self, uid: i64, object_id: &str) -> Result<EncodedCollabV1, anyhow::Error>;
}

#[derive(Clone)]
pub struct RocksdbDiskPlugin {
  uid: i64,
  db: Weak<RocksCollabDB>,
  did_load: Arc<AtomicBool>,
  /// the number of updates on disk when opening the document
  initial_update_count: Arc<AtomicU32>,
  update_count: Arc<AtomicU32>,
  config: CollabPersistenceConfig,
  backup: Option<Arc<dyn RocksdbBackup>>,
}

impl Deref for RocksdbDiskPlugin {
  type Target = Weak<RocksCollabDB>;

  fn deref(&self) -> &Self::Target {
    &self.db
  }
}

impl RocksdbDiskPlugin {
  pub fn new_with_config(
    uid: i64,
    db: Weak<RocksCollabDB>,
    config: CollabPersistenceConfig,
    backup: Option<Arc<dyn RocksdbBackup>>,
  ) -> Self {
    let initial_update_count = Arc::new(AtomicU32::new(0));
    let update_count = Arc::new(AtomicU32::new(0));
    let did_load = Arc::new(AtomicBool::new(false));
    Self {
      db,
      uid,
      did_load,
      initial_update_count,
      update_count,
      config,
      backup,
    }
  }

  pub fn new(uid: i64, db: Weak<RocksCollabDB>, backup: Option<Arc<dyn RocksdbBackup>>) -> Self {
    Self::new_with_config(uid, db, CollabPersistenceConfig::default(), backup)
  }

  fn increase_count(&self) -> u32 {
    self.update_count.fetch_add(1, SeqCst)
  }

  #[instrument(skip_all)]
  fn flush_doc(&self, db: &Arc<RocksCollabDB>, object_id: &str) {
    let _ = db.with_write_txn(|w_db_txn| {
      let doc = make_yrs_doc();
      w_db_txn.load_doc_with_txn(self.uid, object_id, &mut doc.transact_mut())?;
      if let Ok(read_txn) = doc.try_transact() {
        let doc_state = read_txn.encode_state_as_update_v1(&StateVector::default());
        let state_vector = read_txn.state_vector().encode_v1();
        let encoded = EncodedCollabV1::new(state_vector, doc_state);

        w_db_txn.flush_doc(
          self.uid,
          object_id,
          encoded.state_vector.to_vec(),
          encoded.doc_state.to_vec(),
        )?;

        if let Some(backup) = &self.backup {
          backup_doc(self.uid, backup, object_id, encoded);
        }
      }

      Ok(())
    });
  }
}

impl CollabPlugin for RocksdbDiskPlugin {
  fn init(&self, object_id: &str, origin: &CollabOrigin, doc: &Doc) {
    if let Some(db) = self.db.upgrade() {
      let rocksdb_read = db.read_txn();
      // Check the document is exist or not
      if rocksdb_read.is_exist(self.uid, object_id) {
        let mut txn = doc.transact_mut_with(origin.clone());
        // Safety: The document is exist, so it must be loaded successfully.
        let update_count = match rocksdb_read.load_doc_with_txn(self.uid, object_id, &mut txn) {
          Ok(update_count) => {
            self.initial_update_count.store(update_count, SeqCst);
            update_count
          },
          Err(e) => {
            error!("🔴 load doc:{} failed: {}", object_id, e);
            0
          },
        };
        drop(rocksdb_read);
        txn.commit();
        drop(txn);

        if self.config.flush_doc && update_count != 0 && update_count % 20 == 0 {
          self.flush_doc(&db, object_id);
          self.initial_update_count.store(0, SeqCst);
        }
      } else {
        let txn = doc.transact();
        let result = db.with_write_txn(|w_db_txn| {
          w_db_txn.create_new_doc(self.uid, object_id, &txn)?;
          Ok(())
        });

        if let Err(e) = result {
          error!("🔴 create doc for {:?} failed: {}", object_id, e)
        }
      }
    } else {
      tracing::warn!("collab_db is dropped");
    };
  }

  fn did_init(&self, _awareness: &Awareness, _object_id: &str, _last_sync_at: i64) {
    self.did_load.store(true, SeqCst);
  }

  fn receive_update(&self, object_id: &str, _txn: &TransactionMut, update: &[u8]) {
    // Only push update if the doc is loaded
    if !self.did_load.load(SeqCst) {
      return;
    }
    if let Some(db) = self.db.upgrade() {
      let _ = self.increase_count();
      // /Acquire a write transaction to ensure consistency
      let result = db.with_write_txn(|w_db_txn| {
        let _ = w_db_txn.push_update(self.uid, object_id, update)?;
        Ok(())
      });

      if let Err(e) = result {
        error!("🔴Save update failed: {:?}", e);
      }
    } else {
      tracing::warn!("collab_db is dropped");
    };
  }

  fn after_transaction(&self, _object_id: &str, _txn: &mut TransactionMut) {}

  fn reset(&self, object_id: &str) {
    if let Some(db) = self.db.upgrade() {
      if let Err(e) = db.with_write_txn(|w_db_txn| {
        w_db_txn.delete_all_updates(self.uid, object_id)?;
        Ok(())
      }) {
        error!("🔴Reset failed: {:?}", e);
      }
    }
  }

  fn flush(&self, object_id: &str) {
    if let Some(db) = self.db.upgrade() {
      self.flush_doc(&db, object_id);
    }
  }
}

fn backup_doc(
  uid: i64,
  backup: &Arc<dyn RocksdbBackup>,
  object_id: &str,
  encoded: EncodedCollabV1,
) {
  let weak_backup = Arc::downgrade(backup);
  let object_id = object_id.to_string();
  tokio::spawn(async move {
    let _ = tokio::task::spawn_blocking(move || {
      if let Some(backup) = weak_backup.upgrade() {
        match backup.save_doc(uid, &object_id, encoded) {
          Ok(_) => {},
          Err(err) => {
            error!("rocksdb backup save doc failed: {}", err);
          },
        }
      }
    })
    .await;
  });
}
