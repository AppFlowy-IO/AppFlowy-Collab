use crate::cloud_storage::postgres::postgres_db::{PostgresDB, SupabaseDBConfig};
use collab::core::collab::MutexCollab;
use collab::core::origin::CollabOrigin;
use collab::preclude::CollabPlugin;

use std::sync::Arc;
use y_sync::awareness::Awareness;
use yrs::Transaction;

pub struct SupabaseDBPlugin {
  object_id: String,
  local_collab: Arc<MutexCollab>,
  postgres_db: Arc<PostgresDB>,
}

impl SupabaseDBPlugin {
  pub fn new(
    object_id: String,
    local_collab: Arc<MutexCollab>,
    sync_per_secs: u64,
    config: SupabaseDBConfig,
  ) -> Self {
    let postgres_db = PostgresDB::new(object_id.clone(), sync_per_secs, config);

    Self {
      object_id,
      local_collab,
      postgres_db: Arc::new(postgres_db),
    }
  }
}

impl CollabPlugin for SupabaseDBPlugin {
  fn did_init(&self, _awareness: &Awareness, _object_id: &str, _txn: &Transaction) {
    let weak_postgres_db = Arc::downgrade(&self.postgres_db);
    let weak_local_collab = Arc::downgrade(&self.local_collab);
    tokio::spawn(async move {
      if let (Some(postgres_db), Some(local_collab)) =
        (weak_postgres_db.upgrade(), weak_local_collab.upgrade())
      {
        postgres_db.start_sync(local_collab.clone()).await;
      }
    });
  }

  fn receive_local_update(&self, _origin: &CollabOrigin, _object_id: &str, update: &[u8]) {
    self.postgres_db.push_update(update);
  }
}
