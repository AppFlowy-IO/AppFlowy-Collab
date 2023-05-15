#[cfg(feature = "sync")]
mod sync_plugin;

#[cfg(feature = "sync")]
pub mod sync {
  pub use collab_sync::*;

  pub use crate::sync_plugin::*;
}

#[cfg(any(feature = "disk_rocksdb", feature = "disk_sled"))]
mod disk_plugin;

#[cfg(any(feature = "disk_rocksdb", feature = "disk_sled"))]
pub mod disk {
  pub use collab_persistence::*;

  pub use crate::disk_plugin::*;
}

#[cfg(feature = "cloud_storage")]
pub mod cloud_storage;
