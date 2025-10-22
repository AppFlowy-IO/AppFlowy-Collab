pub mod local_storage;

pub mod connect_state;

#[cfg(all(feature = "plugins", not(target_arch = "wasm32")))]
mod native {
  use super::local_storage;
  if_native! {
      pub type CollabKVDB = local_storage::rocksdb::kv_impl::KVTransactionDBRocksdbImpl;
  }
}

#[cfg(all(feature = "plugins", target_arch = "wasm32"))]
mod wasm {
  use super::local_storage;
  if_wasm! {
      pub type CollabKVDB = local_storage::indexeddb::CollabIndexeddb;
  }
}

#[cfg(all(feature = "plugins", not(target_arch = "wasm32")))]
pub use native::CollabKVDB;
#[cfg(all(feature = "plugins", target_arch = "wasm32"))]
pub use wasm::CollabKVDB;
