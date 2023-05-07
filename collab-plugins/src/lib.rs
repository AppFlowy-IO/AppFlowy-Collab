#[cfg(feature = "sync")]
pub mod sync_plugin;

#[cfg(any(feature = "disk_rocksdb", feature = "disk_sled"))]
pub mod disk_plugin;
