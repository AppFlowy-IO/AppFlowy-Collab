use crate::local_storage::indexeddb::kv_impl::CollabIndexeddb;
use crate::local_storage::kv::doc::CollabKVAction;
use collab::core::awareness::Awareness;
use collab::core::origin::CollabOrigin;
use collab::preclude::CollabPlugin;
use collab_entity::CollabType;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering::SeqCst;
use std::sync::{Arc, Weak};
use tracing::error;
use yrs::{Doc, Transact, TransactionMut};

pub struct IndexeddbDiskPlugin {
  uid: i64,
  object_id: String,
  collab_type: CollabType,
  // collab_db: Weak<CollabIndexeddb>,
  did_load: Arc<AtomicBool>,
}

impl IndexeddbDiskPlugin {
  pub fn new(
    uid: i64,
    object_id: String,
    collab_type: CollabType,
    // collab_db: Weak<CollabIndexeddb>,
  ) -> Self {
    let did_load = Arc::new(AtomicBool::new(false));
    Self {
      uid,
      object_id,
      collab_type,
      did_load,
      // collab_db,
    }
  }
}

impl CollabPlugin for IndexeddbDiskPlugin {
  fn init(&self, object_id: &str, origin: &CollabOrigin, doc: &Doc) {
    todo!()
  }
  fn did_init(&self, _awareness: &Awareness, _object_id: &str, _last_sync_at: i64) {
    self.did_load.store(true, SeqCst);
  }
}
