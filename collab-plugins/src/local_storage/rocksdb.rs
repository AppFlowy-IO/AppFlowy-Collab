use std::ops::Deref;
use std::sync::atomic::Ordering::SeqCst;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Weak};

use crate::local_storage::CollabPersistenceConfig;
use collab::core::origin::CollabOrigin;
use collab::preclude::CollabPlugin;
use collab_persistence::doc::YrsDocAction;
use collab_persistence::kv::rocks_kv::RocksCollabDB;
use y_sync::awareness::Awareness;
use yrs::{Doc, Transact, TransactionMut};

#[derive(Clone)]
pub struct RocksdbDiskPlugin {
  uid: i64,
  db: Weak<RocksCollabDB>,
  did_load: Arc<AtomicBool>,
  /// the number of updates on disk when opening the document
  initial_update_count: Arc<AtomicU32>,
  update_count: Arc<AtomicU32>,
  config: CollabPersistenceConfig,
}

impl Deref for RocksdbDiskPlugin {
  type Target = Weak<RocksCollabDB>;

  fn deref(&self) -> &Self::Target {
    &self.db
  }
}

impl RocksdbDiskPlugin {
  pub fn new(uid: i64, db: Weak<RocksCollabDB>) -> Self {
    Self::new_with_config(uid, db, CollabPersistenceConfig::default())
  }

  pub fn new_with_config(
    uid: i64,
    db: Weak<RocksCollabDB>,
    config: CollabPersistenceConfig,
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
    }
  }

  fn increase_count(&self) -> u32 {
    self.update_count.fetch_add(1, SeqCst)
  }
}

impl CollabPlugin for RocksdbDiskPlugin {
  fn init(&self, object_id: &str, original: &CollabOrigin, doc: &Doc) {
    if let Some(db) = self.db.upgrade() {
      let rocksdb_read = db.read_txn();
      let mut txn = doc.transact_mut_with(original.clone());
      // Check the document is exist or not
      if rocksdb_read.is_exist(self.uid, object_id) {
        // Safety: The document is exist, so it must be loaded successfully.
        match rocksdb_read.load_doc_with_txn(self.uid, object_id, &mut txn) {
          Ok(update_count) => {
            self
              .initial_update_count
              .store(update_count, Ordering::SeqCst);
          },
          Err(e) => tracing::error!("🔴 load doc:{} failed: {}", object_id, e),
        }
        drop(rocksdb_read);

        if self.config.flush_doc {
          let _ = db.with_write_txn(|w_db_txn| {
            w_db_txn.flush_doc_with_txn(self.uid, object_id, &txn)?;
            self.initial_update_count.store(0, Ordering::SeqCst);
            Ok(())
          });
        }
      } else {
        // Drop the read txn before write txn
        let result = db.with_write_txn(|w_db_txn| {
          w_db_txn.create_new_doc(self.uid, object_id, &txn)?;
          Ok(())
        });

        if let Err(e) = result {
          tracing::error!("🔴 create doc for {:?} failed: {}", object_id, e)
        }
      }
    } else {
      tracing::warn!("collab_db is dropped");
    };
  }

  fn did_init(&self, _awareness: &Awareness, _object_id: &str) {
    self.did_load.store(true, Ordering::SeqCst);
  }

  fn receive_update(&self, object_id: &str, _txn: &TransactionMut, update: &[u8]) {
    // Only push update if the doc is loaded
    if !self.did_load.load(Ordering::SeqCst) {
      return;
    }
    if let Some(db) = self.db.upgrade() {
      let _ = self.increase_count();
      // /Acquire a write transaction to ensure consistency
      let result = db.with_write_txn(|w_db_txn| {
        tracing::trace!("Receive {} update", object_id);
        let _ = w_db_txn.push_update(self.uid, object_id, update)?;
        Ok(())
      });

      if let Err(e) = result {
        tracing::error!("🔴Save update failed: {:?}", e);
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
        tracing::error!("🔴Reset failed: {:?}", e);
      }
    }
  }
}
