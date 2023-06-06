use std::sync::Arc;

use collab::core::collab::MutexCollab;
use collab::preclude::CollabBuilder;
use collab_plugins::cloud_storage::aws::AWSDynamoDBPlugin;
use collab_plugins::cloud_storage::postgres::SupabaseDBPlugin;
use collab_plugins::cloud_storage::CollabObject;
use collab_plugins::disk::kv::rocks_kv::RocksCollabDB;
use collab_plugins::disk::rocksdb::{CollabPersistenceConfig, RocksdbDiskPlugin};
use collab_plugins::snapshot::{CollabSnapshotPlugin, SnapshotDB};
use parking_lot::RwLock;

use crate::config::{CollabPluginConfig, AWS_ACCESS_KEY_ID, AWS_SECRET_ACCESS_KEY};

#[derive(Clone, Debug)]
pub enum CloudStorageType {
  Local,
  AWS,
  Supabase,
}

pub struct AppFlowyCollabBuilder {
  cloud_storage_type: RwLock<CloudStorageType>,
  snapshot_db: Option<Arc<dyn SnapshotDB>>,
}

impl AppFlowyCollabBuilder {
  pub fn new(
    cloud_storage_type: CloudStorageType,
    snapshot_db: Option<Arc<dyn SnapshotDB>>,
  ) -> Self {
    Self {
      cloud_storage_type: RwLock::new(cloud_storage_type),
      snapshot_db,
    }
  }

  pub fn set_cloud_storage_type(&self, cloud_storage_type: CloudStorageType) {
    *self.cloud_storage_type.write() = cloud_storage_type;
  }

  /// # Arguments
  ///
  /// * `uid`: user id
  /// * `object_id`: the collab object id
  /// * `object_name`: the collab object name. Currently only used to debug.
  /// * `db`: the RocksCollabDB instance.
  ///
  /// returns: Arc<MutexCollab>
  ///
  pub fn build(
    &self,
    uid: i64,
    object_id: &str,
    object_name: &str,
    db: Arc<RocksCollabDB>,
  ) -> Arc<MutexCollab> {
    self.build_with_config(
      uid,
      object_id,
      object_name,
      db,
      &CollabPersistenceConfig::default(),
    )
  }

  pub fn build_with_config(
    &self,
    uid: i64,
    object_id: &str,
    object_name: &str,
    db: Arc<RocksCollabDB>,
    config: &CollabPersistenceConfig,
  ) -> Arc<MutexCollab> {
    let collab = Arc::new(
      CollabBuilder::new(uid, object_id)
        .with_plugin(RocksdbDiskPlugin::new_with_config(uid, db, config.clone()))
        .build(),
    );

    let collab_config = CollabPluginConfig::from_env();
    let cloud_storage_type = self.cloud_storage_type.read().clone();
    tracing::trace!("collab cloud storage type: {:?}", cloud_storage_type);
    match cloud_storage_type {
      CloudStorageType::AWS => {
        if let Some(config) = collab_config.aws_config() {
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
            tracing::debug!("add aws plugin: {:?}", self.cloud_storage_type);
          }
        }
      },
      CloudStorageType::Supabase => {
        if let Some(config) = collab_config.supabase_config() {
          if config.collab_table_config.enable {
            let collab_object = CollabObject::new(object_id.to_string()).with_name(object_name);
            let plugin = SupabaseDBPlugin::new(collab_object, collab.clone(), 10, config.clone());
            collab.lock().add_plugin(Arc::new(plugin));
            tracing::trace!("add supabase plugin: {:?}", self.cloud_storage_type);
          }
        }
      },
      CloudStorageType::Local => {},
    }

    if let Some(snapshot_db) = &self.snapshot_db {
      if config.enable_snapshot {
        let snapshot_plugin = CollabSnapshotPlugin::new(
          uid,
          snapshot_db.clone(),
          collab.clone(),
          config.snapshot_per_update,
        );
        tracing::trace!("add snapshot plugin: {}", object_id);
        collab.lock().add_plugin(Arc::new(snapshot_plugin));
      }
    }

    collab.lock().initialize();
    collab
  }
}
