use crate::error::WSError;
use std::future::Future;
use std::pin::Pin;

use tokio::net::TcpStream;
use tokio_retry::Action;
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};

pub(crate) struct ConnectAction {
  addr: String,
}

impl ConnectAction {
  pub fn new(addr: String) -> Self {
    Self { addr }
  }
}

impl Action for ConnectAction {
  type Future = Pin<Box<dyn Future<Output = Result<Self::Item, Self::Error>> + Send + Sync>>;
  type Item = WebSocketStream<MaybeTlsStream<TcpStream>>;
  type Error = WSError;

  fn run(&mut self) -> Self::Future {
    let cloned_addr = self.addr.clone();
    Box::pin(async move {
      match connect_async(&cloned_addr).await {
        Ok((stream, response)) => {
          tracing::trace!("{:?}", response);
          Ok(stream)
        },
        Err(e) => {
          //
          tracing::error!("🔴connect error: {:?}", e.to_string());
          Err(e.into())
        },
      }
    })
  }
}
