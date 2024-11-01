use std::ops::{Deref, DerefMut};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use collab::core::collab::DataSource;
use collab::preclude::CollabBuilder;
use collab_entity::CollabType;
use collab_plugins::local_storage::rocksdb::rocksdb_plugin::RocksdbDiskPlugin;
use collab_plugins::CollabKVDB;
use collab_user::core::{RemindersChangeSender, UserAwareness, UserAwarenessNotifier};
use tempfile::TempDir;
use tokio::sync::broadcast::Receiver;
use tokio::time::timeout;
use uuid::Uuid;

pub struct UserAwarenessTest {
  pub user_awareness: UserAwareness,
  #[allow(dead_code)]
  pub reminder_change_tx: RemindersChangeSender,
}

impl Deref for UserAwarenessTest {
  type Target = UserAwareness;

  fn deref(&self) -> &Self::Target {
    &self.user_awareness
  }
}

impl DerefMut for UserAwarenessTest {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.user_awareness
  }
}

impl UserAwarenessTest {
  pub fn new(uid: i64) -> Self {
    let workspace_id = Uuid::new_v4().to_string();
    let tempdir = TempDir::new().unwrap();

    let path = tempdir.into_path();
    let db = Arc::new(CollabKVDB::open(path.clone()).unwrap());
    let id = uuid::Uuid::new_v4().to_string();
    let disk_plugin = RocksdbDiskPlugin::new(
      uid,
      workspace_id,
      id,
      CollabType::UserAwareness,
      Arc::downgrade(&db),
    );

    let mut collab = CollabBuilder::new(1, uid.to_string(), DataSource::Disk(None))
      .with_plugin(disk_plugin)
      .with_device_id("1")
      .build()
      .unwrap();
    collab.initialize();

    let (reminder_change_tx, _) = tokio::sync::broadcast::channel(100);
    let notifier = UserAwarenessNotifier {
      reminder_change_tx: reminder_change_tx.clone(),
    };
    let user_awareness = UserAwareness::create(collab, Some(notifier)).unwrap();
    Self {
      user_awareness,
      reminder_change_tx,
    }
  }
}

pub async fn receive_with_timeout<T>(receiver: &mut Receiver<T>, duration: Duration) -> Result<T>
where
  T: Clone,
{
  let res = timeout(duration, receiver.recv()).await??;
  Ok(res)
}
