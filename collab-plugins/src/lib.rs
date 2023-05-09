#[cfg(feature = "sync")]
mod sync_plugin;

#[cfg(feature = "sync")]
pub mod sync {
  pub use crate::sync_plugin::*;
  pub use collab_sync::*;
}

#[cfg(any(feature = "disk_rocksdb", feature = "disk_sled"))]
mod disk_plugin;

#[cfg(any(feature = "disk_rocksdb", feature = "disk_sled"))]
pub mod disk {
  pub use crate::disk_plugin::*;
  pub use collab_persistence::*;
}
