use std::fmt::Debug;
use std::sync::Arc;

use collab::core::collab::MutexCollab;
use collab::preclude::CollabBuilder;
use collab_plugins::cloud_storage::aws::AWSDynamoDBPlugin;
use collab_plugins::cloud_storage::postgres::SupabaseDBPlugin;
use collab_plugins::cloud_storage::{CollabObject, RemoteCollabStorage};
use collab_plugins::disk::kv::rocks_kv::RocksCollabDB;
use collab_plugins::disk::rocksdb::{CollabPersistenceConfig, RocksdbDiskPlugin};
use collab_plugins::snapshot::{CollabSnapshotPlugin, SnapshotPersistence};
use parking_lot::RwLock;

use crate::config::{CollabPluginConfig, AWS_ACCESS_KEY_ID, AWS_SECRET_ACCESS_KEY};

#[derive(Clone, Debug)]
pub enum CollabStorageType {
  Local,
  AWS,
  Supabase,
}

pub trait CollabStorageProvider: Send + Sync + 'static {
  fn storage_type(&self) -> CollabStorageType;
  fn get_storage(&self, storage_type: &CollabStorageType) -> Option<Arc<dyn RemoteCollabStorage>>;
}

impl<T> CollabStorageProvider for Arc<T>
where
  T: CollabStorageProvider,
{
  fn storage_type(&self) -> CollabStorageType {
    (**self).storage_type()
  }

  fn get_storage(&self, storage_type: &CollabStorageType) -> Option<Arc<dyn RemoteCollabStorage>> {
    (**self).get_storage(storage_type)
  }
}

pub struct AppFlowyCollabBuilder {
  cloud_storage: RwLock<Arc<dyn CollabStorageProvider>>,
  snapshot_persistence: Option<Arc<dyn SnapshotPersistence>>,
}

impl AppFlowyCollabBuilder {
  pub fn new<T: CollabStorageProvider>(
    cloud_storage: T,
    snapshot_persistence: Option<Arc<dyn SnapshotPersistence>>,
  ) -> Self {
    Self {
      cloud_storage: RwLock::new(Arc::new(cloud_storage)),
      snapshot_persistence,
    }
  }

  /// Create a new collab builder with default config.
  /// The [MutexCollab] will be create if the object is not exist. So, if you need to check
  /// the object is exist or not. You should use the transaction returned by the [read_txn] method of
  /// [RocksCollabDB], and calling [is_exist] method.
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

  /// Create a new collab builder with custom config.
  /// The [MutexCollab] will be create if the object is not exist. So, if you need to check
  /// the object is exist or not. You should use the transaction returned by the [read_txn] method of
  /// [RocksCollabDB], and calling [is_exist] method.
  ///
  pub fn build_with_config(
    &self,
    uid: i64,
    object_id: &str,
    object_name: &str,
    collab_db: Arc<RocksCollabDB>,
    config: &CollabPersistenceConfig,
  ) -> Arc<MutexCollab> {
    let collab = Arc::new(
      CollabBuilder::new(uid, object_id)
        .with_plugin(RocksdbDiskPlugin::new_with_config(
          uid,
          collab_db.clone(),
          config.clone(),
        ))
        .build(),
    );

    let collab_config = CollabPluginConfig::from_env();
    let cloud_storage = self.cloud_storage.read();
    let cloud_storage_type = cloud_storage.storage_type();
    tracing::trace!("collab storage type: {:?}", cloud_storage_type);
    match cloud_storage_type {
      CollabStorageType::AWS => {
        if let Some(config) = collab_config.aws_config() {
          if !config.enable {
            std::env::remove_var(AWS_ACCESS_KEY_ID);
            std::env::remove_var(AWS_SECRET_ACCESS_KEY);
          } else {
            std::env::set_var(AWS_ACCESS_KEY_ID, &config.access_key_id);
            std::env::set_var(AWS_SECRET_ACCESS_KEY, &config.secret_access_key);
            let plugin = AWSDynamoDBPlugin::new(
              object_id.to_string(),
              Arc::downgrade(&collab),
              10,
              config.region.clone(),
            );
            collab.lock().add_plugin(Arc::new(plugin));
            tracing::debug!("add aws plugin: {:?}", cloud_storage_type);
          }
        }
      },
      CollabStorageType::Supabase => {
        let collab_object = CollabObject::new(object_id.to_string()).with_name(object_name);
        let local_collab_storage = collab_db.clone();
        if let Some(remote_collab_storage) = cloud_storage.get_storage(&cloud_storage_type) {
          let plugin = SupabaseDBPlugin::new(
            uid,
            collab_object,
            Arc::downgrade(&collab),
            2,
            remote_collab_storage,
            local_collab_storage,
          );
          collab.lock().add_plugin(Arc::new(plugin));
          tracing::trace!("add supabase plugin: {:?}", cloud_storage_type);
        }
      },
      CollabStorageType::Local => {},
    }

    if let Some(snapshot_persistence) = &self.snapshot_persistence {
      if config.enable_snapshot {
        let collab_object = CollabObject::new(object_id.to_string()).with_name(object_name);
        let snapshot_plugin = CollabSnapshotPlugin::new(
          uid,
          collab_object,
          snapshot_persistence.clone(),
          collab_db,
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

pub struct DefaultCollabStorageProvider();
impl CollabStorageProvider for DefaultCollabStorageProvider {
  fn storage_type(&self) -> CollabStorageType {
    CollabStorageType::Local
  }

  fn get_storage(&self, _storage_type: &CollabStorageType) -> Option<Arc<dyn RemoteCollabStorage>> {
    None
  }
}
