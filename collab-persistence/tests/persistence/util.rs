use std::path::PathBuf;
use std::sync::Once;

use collab_persistence::kv::rocks_kv::RocksCollabDB;
use collab_persistence::kv::sled_lv::SledCollabDB;

use tempfile::TempDir;
use tracing_subscriber::{fmt::Subscriber, util::SubscriberInitExt, EnvFilter};

pub fn sled_db() -> (PathBuf, SledCollabDB) {
  setup_log();

  let tempdir = TempDir::new().unwrap();
  let path = tempdir.into_path();
  let cloned_path = path.clone();
  (path, SledCollabDB::open(cloned_path).unwrap())
}

pub fn rocks_db(_uid: i64) -> (PathBuf, RocksCollabDB) {
  setup_log();

  let tempdir = TempDir::new().unwrap();
  let path = tempdir.into_path();
  let cloned_path = path.clone();
  (path, RocksCollabDB::open(cloned_path).unwrap())
}

fn setup_log() {
  static START: Once = Once::new();
  START.call_once(|| {
    std::env::set_var("RUST_LOG", "collab_persistence=trace");
    let subscriber = Subscriber::builder()
      .with_env_filter(EnvFilter::from_default_env())
      .with_ansi(true)
      .finish();
    subscriber.try_init().unwrap();
  });
}
