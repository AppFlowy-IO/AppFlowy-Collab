mod client;
mod conn;
mod script;
mod server;
pub use client::*;
pub use conn::*;
pub use script::*;
pub use server::*;
use std::time::Duration;

pub async fn wait_a_sec() {
  tokio::time::sleep(Duration::from_secs(1)).await;
}
