use std::ops::Deref;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use collab::error::CollabError;
use collab::preclude::CollabPlugin;
use collab_persistence::doc::YrsDocAction;
use collab_persistence::kv::rocks_kv::RocksCollabDB;

use collab_sync::server::CollabId;
use y_sync::awareness::Awareness;
use yrs::{Transaction, TransactionMut};

#[derive(Clone)]
pub struct RocksdbServerDiskPlugin {
  collab_id: CollabId,
  db: Arc<RocksCollabDB>,
  did_load: Arc<AtomicBool>,
}

impl Deref for RocksdbServerDiskPlugin {
  type Target = Arc<RocksCollabDB>;

  fn deref(&self) -> &Self::Target {
    &self.db
  }
}

impl RocksdbServerDiskPlugin {
  pub fn new(collab_id: CollabId, db: Arc<RocksCollabDB>) -> Result<Self, CollabError> {
    let did_load = Arc::new(AtomicBool::new(false));
    Ok(Self {
      collab_id,
      db,
      did_load,
    })
  }
}

impl CollabPlugin for RocksdbServerDiskPlugin {
  fn init(&self, object_id: &str, txn: &mut TransactionMut) {
    let r_db_txn = self.db.read_txn();

    // Check the document is exist or not
    if r_db_txn.is_exist(self.collab_id, object_id) {
      // Safety: The document is exist, so it must be loaded successfully.
      let _ = r_db_txn.load_doc(self.collab_id, object_id, txn).unwrap();
      drop(r_db_txn);
    } else {
      // Drop the read txn before write txn
      let result = self.db.with_write_txn(|w_db_txn| {
        w_db_txn.create_new_doc(self.collab_id, object_id, txn)?;
        Ok(())
      });

      if let Err(e) = result {
        tracing::warn!("[🦀Collab] => create doc for {:?} failed: {}", object_id, e)
      }
    }
  }
  fn did_init(&self, _awareness: &Awareness, _object_id: &str, _txn: &Transaction) {
    self.did_load.store(true, Ordering::SeqCst);
  }

  fn receive_update(&self, object_id: &str, _txn: &TransactionMut, update: &[u8]) {
    // Only push update if the doc is loaded
    if !self.did_load.load(Ordering::SeqCst) {
      return;
    }
    // /Acquire a write transaction to ensure consistency
    let result = self.db.with_write_txn(|w_db_txn| {
      let _ = w_db_txn.push_update(self.collab_id, object_id, update)?;
      Ok(())
    });

    if let Err(e) = result {
      tracing::error!("🔴Save update failed: {:?}", e);
    }
  }
}
