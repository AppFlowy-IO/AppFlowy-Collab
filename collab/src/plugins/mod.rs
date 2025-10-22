pub mod local_storage;

pub mod connect_state;

#[cfg(feature = "plugins")]
pub type CollabKVDB = local_storage::rocksdb::kv_impl::KVTransactionDBRocksdbImpl;
