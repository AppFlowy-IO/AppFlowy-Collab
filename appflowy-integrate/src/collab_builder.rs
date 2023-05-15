use std::sync::Arc;

use collab::core::collab::MutexCollab;
use collab::preclude::CollabBuilder;
// use collab_plugins::cloud_storage_plugin::AWSDynamoDBPlugin;
use collab_plugins::disk::kv::rocks_kv::RocksCollabDB;
use collab_plugins::disk::rocksdb::{Config, RocksdbDiskPlugin};

use crate::config::AppFlowyCollabConfig;

pub struct AppFlowyCollabBuilder {
  #[allow(dead_code)]
  config: AppFlowyCollabConfig,
}

impl AppFlowyCollabBuilder {
  pub fn new(config: AppFlowyCollabConfig) -> Self {
    Self { config }
  }

  pub fn build(&self, uid: i64, object_id: &str, db: Arc<RocksCollabDB>) -> Arc<MutexCollab> {
    self.build_with_config(uid, object_id, db, &Config::default())
  }

  pub fn build_with_config(
    &self,
    uid: i64,
    object_id: &str,
    db: Arc<RocksCollabDB>,
    config: &Config,
  ) -> Arc<MutexCollab> {
    let collab = Arc::new(
      CollabBuilder::new(uid, object_id)
        .with_plugin(RocksdbDiskPlugin::new_with_config(uid, db, config.clone()))
        .build(),
    );

    // if self.config.aws_config.is_some() {
    //   if let Ok(plugin) = AWSDynamoDBPlugin::new(object_id.to_string(), collab.clone(), 5) {
    //     collab.lock().add_plugin(Arc::new(plugin));
    //   }
    // }

    collab.lock().initial();
    collab
  }
}
