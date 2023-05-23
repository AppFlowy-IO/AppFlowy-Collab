use collab::core::collab::MutexCollab;
use collab::core::origin::CollabOrigin;
use collab::preclude::CollabPlugin;

use crate::cloud_storage::postgres::postgres_db::PostgresDB;
use crate::cloud_storage::postgres::SupabaseDBConfig;
use parking_lot::RwLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use y_sync::awareness::Awareness;
use yrs::Transaction;

pub struct SupabaseDBPlugin {
  local_collab: Arc<MutexCollab>,
  postgres_db: Arc<PostgresDB>,
  pending_updates: Arc<RwLock<Vec<Vec<u8>>>>,
  is_first_sync_done: Arc<AtomicBool>,
}

impl SupabaseDBPlugin {
  pub fn new(
    object_id: String,
    local_collab: Arc<MutexCollab>,
    sync_per_secs: u64,
    config: SupabaseDBConfig,
  ) -> Self {
    let postgres_db = PostgresDB::new(object_id, sync_per_secs, config);
    let pending_updates = Arc::new(RwLock::new(Vec::new()));
    let is_first_sync_done = Arc::new(AtomicBool::new(false));
    Self {
      local_collab,
      postgres_db: Arc::new(postgres_db),
      pending_updates,
      is_first_sync_done,
    }
  }
}

impl CollabPlugin for SupabaseDBPlugin {
  fn did_init(&self, _awareness: &Awareness, _object_id: &str, _txn: &Transaction) {
    let weak_postgres_db = Arc::downgrade(&self.postgres_db);
    let weak_local_collab = Arc::downgrade(&self.local_collab);
    let weak_pending_updates = Arc::downgrade(&self.pending_updates);
    let weak_is_first_sync_done = Arc::downgrade(&self.is_first_sync_done);

    tokio::spawn(async move {
      if let (
        Some(postgres_db),
        Some(local_collab),
        Some(pending_updates),
        Some(is_first_sync_done),
      ) = (
        weak_postgres_db.upgrade(),
        weak_local_collab.upgrade(),
        weak_pending_updates.upgrade(),
        weak_is_first_sync_done.upgrade(),
      ) {
        postgres_db.start_sync(local_collab.clone()).await;
        for update in &*pending_updates.read() {
          postgres_db.push_update(update);
        }

        is_first_sync_done.store(true, Ordering::SeqCst)
      }
    });
  }

  fn receive_local_update(&self, _origin: &CollabOrigin, _object_id: &str, update: &[u8]) {
    if self.is_first_sync_done.load(Ordering::SeqCst) {
      self.postgres_db.push_update(update);
    } else {
      self.pending_updates.write().push(update.to_vec());
    }
  }
}
