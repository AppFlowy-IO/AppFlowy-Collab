use std::ops::Deref;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use collab::preclude::CollabBuilder;
use collab_plugins::kv::rocks_kv::RocksCollabDB;
use collab_plugins::local_storage::rocksdb::RocksdbDiskPlugin;
use collab_user::core::{
  MutexUserAwareness, RemindersChangeSender, UserAwareness, UserAwarenessNotifier,
};
use tempfile::TempDir;
use tokio::sync::broadcast::Receiver;
use tokio::time::timeout;

#[derive(Clone)]
pub struct UserAwarenessTest {
  user_awareness: MutexUserAwareness,
  #[allow(dead_code)]
  cleaner: Arc<Cleaner>,
  #[allow(dead_code)]
  pub reminder_change_tx: RemindersChangeSender,
}

impl Deref for UserAwarenessTest {
  type Target = MutexUserAwareness;

  fn deref(&self) -> &Self::Target {
    &self.user_awareness
  }
}

impl UserAwarenessTest {
  pub async fn new(uid: i64) -> Self {
    let tempdir = TempDir::new().unwrap();

    let path = tempdir.into_path();
    let db = Arc::new(RocksCollabDB::open(path.clone()).unwrap());
    let disk_plugin = RocksdbDiskPlugin::new(uid, Arc::downgrade(&db));
    let cleaner: Cleaner = Cleaner::new(path);

    let collab = CollabBuilder::new(1, uid.to_string())
      .with_plugin(disk_plugin)
      .with_device_id("1")
      .build()
      .unwrap();
    collab.lock().initialize();

    let (reminder_change_tx, _) = tokio::sync::broadcast::channel(100);
    let notifier = UserAwarenessNotifier {
      reminder_change_tx: reminder_change_tx.clone(),
    };
    let user_awareness = UserAwareness::create(Arc::new(collab), Some(notifier));
    Self {
      user_awareness: MutexUserAwareness::new(user_awareness),
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
