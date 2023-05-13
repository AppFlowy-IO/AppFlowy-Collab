mod client;
mod conn;
mod script;
mod server;
pub use client::*;
pub use conn::*;
pub use script::*;
pub use server::*;
use std::time::Duration;

pub async fn wait_one_sec() {
  tokio::time::sleep(Duration::from_secs(1)).await;
}

pub async fn wait_five_sec() {
  tokio::time::sleep(Duration::from_secs(5)).await;
}
