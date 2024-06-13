use std::ops::Deref;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::Result;
use collab::preclude::CollabBuilder;
use collab_entity::CollabType;
use collab_plugins::local_storage::rocksdb::rocksdb_plugin::RocksdbDiskPlugin;
use collab_plugins::CollabKVDB;
use collab_user::core::{RemindersChangeSender, UserAwareness, UserAwarenessNotifier};
use tempfile::TempDir;
use tokio::sync::broadcast::Receiver;
use tokio::time::timeout;

#[derive(Clone)]
pub struct UserAwarenessTest {
  user_awareness: Arc<UserAwareness>,
  #[allow(dead_code)]
  cleaner: Arc<Cleaner>,
  #[allow(dead_code)]
  pub reminder_change_tx: RemindersChangeSender,
}

impl Deref for UserAwarenessTest {
  type Target = Arc<UserAwareness>;

  fn deref(&self) -> &Self::Target {
    &self.user_awareness
  }
}

impl UserAwarenessTest {
  pub async fn new(uid: i64) -> Self {
    let tempdir = TempDir::new().unwrap();

    let path = tempdir.into_path();
    let db = Arc::new(CollabKVDB::open(path.clone()).unwrap());
    let id = uuid::Uuid::new_v4().to_string();
    let disk_plugin = RocksdbDiskPlugin::new(
      uid,
      id,
      CollabType::UserAwareness,
      Arc::downgrade(&db),
      None,
    );
    let cleaner: Cleaner = Cleaner::new(path);

    let mut collab = CollabBuilder::new(1, uid.to_string())
      .with_plugin(disk_plugin)
      .with_device_id("1")
      .build()
      .unwrap();
    collab.initialize();
    let collab = Arc::new(Mutex::new(collab));

    let (reminder_change_tx, _) = tokio::sync::broadcast::channel(100);
    let notifier = UserAwarenessNotifier {
      reminder_change_tx: reminder_change_tx.clone(),
    };
    let user_awareness = UserAwareness::create(collab, Some(notifier));
    Self {
      user_awareness: Arc::new(user_awareness),
      cleaner: Arc::new(cleaner),
      reminder_change_tx,
    }
  }
}

struct Cleaner(PathBuf);

impl Cleaner {
  fn new(dir: PathBuf) -> Self {
    Cleaner(dir)
  }

  fn cleanup(dir: &PathBuf) {
    let _ = std::fs::remove_dir_all(dir);
  }
}

impl Drop for Cleaner {
  fn drop(&mut self) {
    Self::cleanup(&self.0)
  }
}

pub async fn receive_with_timeout<T>(receiver: &mut Receiver<T>, duration: Duration) -> Result<T>
where
  T: Clone,
{
  let res = timeout(duration, receiver.recv()).await??;
  Ok(res)
}
