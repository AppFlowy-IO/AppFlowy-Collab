#[cfg(any(feature = "rocksdb_plugin", feature = "disk_sled"))]
pub use collab_persistence::*;

mod ws_sync;

pub mod sync {
  pub use collab_sync::*;

  pub use crate::ws_sync::*;
}

pub mod local_storage;

pub mod cloud_storage;

#[cfg(feature = "snapshot_plugin")]
pub mod snapshot;
