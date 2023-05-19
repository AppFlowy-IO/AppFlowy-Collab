use std::sync::Arc;

use collab::core::collab::MutexCollab;
use collab::preclude::CollabBuilder;
use collab_plugins::cloud_storage::aws::{is_enable_aws_dynamodb, AWSDynamoDBPlugin};
use collab_plugins::cloud_storage::postgres::SupabasePostgresDBPlugin;
// use collab_plugins::cloud_storage_plugin::AWSDynamoDBPlugin;
use collab_plugins::disk::kv::rocks_kv::RocksCollabDB;
use collab_plugins::disk::rocksdb::{Config, RocksdbDiskPlugin};

use crate::config::{AppFlowyCollabConfig, AWS_ACCESS_KEY_ID, AWS_SECRET_ACCESS_KEY};

pub struct AppFlowyCollabBuilder {
  #[allow(dead_code)]
  collab_config: AppFlowyCollabConfig,
}

impl AppFlowyCollabBuilder {
  pub fn new(collab_config: AppFlowyCollabConfig) -> Self {
    Self { collab_config }
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

    if let Some(config) = self.collab_config.aws_config() {
      if !config.enable {
        std::env::remove_var(AWS_ACCESS_KEY_ID);
        std::env::remove_var(AWS_SECRET_ACCESS_KEY);
      } else {
        std::env::set_var(AWS_ACCESS_KEY_ID, &config.access_key_id);
        std::env::set_var(AWS_SECRET_ACCESS_KEY, &config.secret_access_key);
        let plugin = AWSDynamoDBPlugin::new(
          object_id.to_string(),
          collab.clone(),
          5,
          config.region.clone(),
        );
        collab.lock().add_plugin(Arc::new(plugin));
      }
    }

    if let Some(config) = self.collab_config.supabase_config() {
      if config.enable {
        let plugin =
          SupabasePostgresDBPlugin::new(object_id.to_string(), collab.clone(), 10, config.clone());
        collab.lock().add_plugin(Arc::new(plugin));
      }
    }

    // let aws_dynamodb_plugin = AWSDynamoDBPlugin::new(
    //   object_id.to_string(),
    //   collab.clone(),
    //   5,
    //   "ap-southeast-2".to_string(),
    // );
    // collab.lock().add_plugin(Arc::new(aws_dynamodb_plugin));

    collab.lock().initial();
    collab
  }
}
