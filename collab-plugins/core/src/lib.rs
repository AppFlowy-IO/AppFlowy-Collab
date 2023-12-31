use crate::local_storage::rocksdb::kv_impl::RocksStore;

pub mod local_storage;

#[cfg(feature = "postgres_plugin")]
pub mod cloud_storage;
pub mod network_state;

#[cfg(feature = "rocksdb_plugin")]
pub type CollabKVDB = RocksStore;
