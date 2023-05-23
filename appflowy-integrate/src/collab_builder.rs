use std::sync::Arc;

use collab::core::collab::MutexCollab;
use collab::preclude::CollabBuilder;
use collab_plugins::cloud_storage::aws::AWSDynamoDBPlugin;
use collab_plugins::cloud_storage::postgres::SupabaseDBPlugin;
use collab_plugins::disk::kv::rocks_kv::RocksCollabDB;
use collab_plugins::disk::rocksdb::{CollabPersistenceConfig, RocksdbDiskPlugin};

use crate::config::{CollabPluginConfig, AWS_ACCESS_KEY_ID, AWS_SECRET_ACCESS_KEY};

pub enum CloudStorageType {
  Local,
  AWS,
  Supabase,
}

pub struct AppFlowyCollabBuilder {
  #[allow(dead_code)]
  collab_config: CollabPluginConfig,
  cloud_storage_type: CloudStorageType,
}

impl AppFlowyCollabBuilder {
  pub fn new(cloud_storage_type: CloudStorageType) -> Self {
    Self {
      collab_config: CollabPluginConfig::from_env(),
      cloud_storage_type,
    }
  }

  pub fn build(&self, uid: i64, object_id: &str, db: Arc<RocksCollabDB>) -> Arc<MutexCollab> {
    self.build_with_config(uid, object_id, db, &CollabPersistenceConfig::default())
  }

  pub fn build_with_config(
    &self,
    uid: i64,
    object_id: &str,
    db: Arc<RocksCollabDB>,
    config: &CollabPersistenceConfig,
  ) -> Arc<MutexCollab> {
    let collab = Arc::new(
      CollabBuilder::new(uid, object_id)
        .with_plugin(RocksdbDiskPlugin::new_with_config(uid, db, config.clone()))
        .build(),
    );

    match self.cloud_storage_type {
      CloudStorageType::AWS => {
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
              10,
              config.region.clone(),
            );
            collab.lock().add_plugin(Arc::new(plugin));
          }
        }
      },
      CloudStorageType::Supabase => {
        if let Some(config) = self.collab_config.supabase_config() {
          if config.update_table_config.enable {
            let plugin =
              SupabaseDBPlugin::new(object_id.to_string(), collab.clone(), 10, config.clone());
            collab.lock().add_plugin(Arc::new(plugin));
          }
        }
      },
      CloudStorageType::Local => {},
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
