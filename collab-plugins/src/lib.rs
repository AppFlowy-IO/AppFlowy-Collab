pub mod local_storage;

#[macro_export]
macro_rules! if_native {
    ($($item:item)*) => {$(
        #[cfg(not(target_arch = "wasm32"))]
        $item
    )*}
}

#[macro_export]
macro_rules! if_wasm {
    ($($item:item)*) => {$(
        #[cfg(target_arch = "wasm32")]
        $item
    )*}
}

#[cfg(all(feature = "postgres_plugin", not(target_arch = "wasm32")))]
pub mod cloud_storage;
pub mod connect_state;

if_native! {
    pub type CollabKVDB = local_storage::rocksdb::kv_impl::KVTransactionDBRocksdbImpl;
}

if_wasm! {
    pub type CollabKVDB = local_storage::indexeddb::CollabIndexeddb;
}
