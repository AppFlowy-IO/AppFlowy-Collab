pub mod local_storage;

#[cfg(all(feature = "postgres_plugin", not(target_arch = "wasm32")))]
pub mod cloud_storage;

pub mod connect_state;

if_native! {
    pub type CollabKVDB = local_storage::rocksdb::kv_impl::KVTransactionDBRocksdbImpl;
}

if_wasm! {
    pub type CollabKVDB = local_storage::indexeddb::CollabIndexeddb;
}
