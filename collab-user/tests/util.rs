use std::ops::Deref;
use std::path::PathBuf;
use std::sync::Arc;

use collab::preclude::CollabBuilder;
use collab_plugins::kv::rocks_kv::RocksCollabDB;
use collab_plugins::local_storage::rocksdb::RocksdbDiskPlugin;
use collab_user::core::{RemindersChangeReceiver, UserAwareness, UserAwarenessNotifier};
use tempfile::TempDir;

pub struct UserAwarenessTest {
  user_awareness: UserAwareness,
  #[allow(dead_code)]
  cleaner: Cleaner,
  #[allow(dead_code)]
  reminder_change_rx: RemindersChangeReceiver,
}

impl Deref for UserAwarenessTest {
  type Target = UserAwareness;

  fn deref(&self) -> &Self::Target {
    &self.user_awareness
  }
}

impl UserAwarenessTest {
  pub fn new(uid: i64) -> Self {
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

    let (tx, rx) = tokio::sync::broadcast::channel(100);
    let notifier = UserAwarenessNotifier {
      reminder_change_tx: tx,
    };
    let user_awareness = UserAwareness::create(Arc::new(collab), Some(notifier));
    Self {
      user_awareness,
      cleaner,
      reminder_change_rx: rx,
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
