pub mod kv;

#[cfg(not(target_arch = "wasm32"))]
pub mod rocksdb;

#[cfg(target_arch = "wasm32")]
pub mod indexeddb;

mod storage_config;

pub use storage_config::*;
