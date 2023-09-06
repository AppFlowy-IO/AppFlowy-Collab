mod broadcast;
mod client;
mod collab_id_gen;
mod conn;
mod rocksdb_server;
mod script;
mod server;
mod sync;

pub use broadcast::*;
pub use client::*;
pub use collab_id_gen::*;
pub use conn::*;
pub use rocksdb_server::*;
pub use script::*;
pub use server::*;
use std::time::Duration;
pub use sync::*;

pub async fn wait_one_sec() {
  tokio::time::sleep(Duration::from_secs(1)).await;
}

#[allow(dead_code)]
pub async fn wait_five_sec() {
  tokio::time::sleep(Duration::from_secs(5)).await;
}
