pub mod local_storage;

#[cfg(feature = "postgres_plugin")]
pub mod cloud_storage;
pub mod connect_state;

#[cfg(not(target_arch = "wasm32"))]
use crate::local_storage::rocksdb::kv_impl::RocksStore;
#[cfg(not(target_arch = "wasm32"))]
pub type CollabKVDB = RocksStore;
