#[cfg(feature = "sync")]
mod ws_sync;

#[cfg(feature = "sync")]
pub mod sync {
  pub use collab_sync::*;

  pub use crate::ws_sync::*;
}

#[cfg(any(feature = "disk_rocksdb", feature = "disk_sled"))]
mod local_storage;

#[cfg(any(feature = "disk_rocksdb", feature = "disk_sled"))]
pub mod disk {
  pub use collab_persistence::*;

  pub use crate::local_storage::*;
}

#[cfg(any(feature = "aws_storage", feature = "postgres_storage"))]
pub mod cloud_storage;

pub mod snapshot;
