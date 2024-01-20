use wasm_bindgen::JsValue;

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
    use wasm_bindgen::prelude::wasm_bindgen;
    pub type CollabKVDB = local_storage::indexeddb::kv_impl::CollabIndexeddb;
    #[wasm_bindgen]
    extern "C" {
        fn get_current_timestamp() ->  wasm_bindgen::JsValue;
    }
}
