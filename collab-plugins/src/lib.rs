#[cfg(any(feature = "rocksdb_plugin"))]
pub use collab_persistence::*;

mod sync_plugin;

pub mod sync {
  pub use crate::sync_plugin::*;
}

pub mod local_storage;

pub mod cloud_storage;

#[cfg(feature = "snapshot_plugin")]
pub mod snapshot;
