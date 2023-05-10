use crate::error::WSError;
use crate::msg::{BusinessID, WSMessage};
use crate::retry::ConnectAction;
use crate::WSMessageHandler;
use futures_util::{SinkExt, StreamExt};

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Weak};
use std::time::Duration;

use tokio::sync::broadcast::{channel, Sender};
use tokio::sync::{Mutex, RwLock};
use tokio_retry::strategy::FixedInterval;
use tokio_retry::Retry;
use tokio_tungstenite::MaybeTlsStream;

pub struct WSClient {
  addr: String,
  state: Mutex<ConnectState>,
  sender: Sender<WSMessage>,
  handlers: Arc<RwLock<HashMap<BusinessID, Weak<WSMessageHandler>>>>,
}

impl WSClient {
  pub fn new(addr: String, buffer_capacity: usize) -> Self {
    let (sender, _) = channel(buffer_capacity);
    let state = Mutex::new(ConnectState::Disconnected);
    let handlers = Arc::new(RwLock::new(HashMap::new()));
    WSClient {
      addr,
      state,
      sender,
      handlers,
    }
  }

  pub async fn connect(&self) -> Result<Option<SocketAddr>, WSError> {
    self.set_state(ConnectState::Connecting).await;
    let retry_strategy = FixedInterval::new(Duration::from_secs(2)).take(3);
    let action = ConnectAction::new(self.addr.clone());
    let stream = Retry::spawn(retry_strategy, action).await?;
    let addr = match stream.get_ref() {
      MaybeTlsStream::Plain(s) => s.local_addr().ok(),
      _ => None,
    };

    let (mut sink, mut stream) = stream.split();

    self.set_state(ConnectState::Connected).await;
    let weak_handlers = Arc::downgrade(&self.handlers);
    // Receive messages from the websocket, and send them to the handlers.
    tokio::spawn(async move {
      while let Some(Ok(msg)) = stream.next().await {
        if let Ok(msg) = WSMessage::try_from(&msg) {
          if let Some(handlers) = weak_handlers.upgrade() {
            if let Some(handler) = handlers
              .read()
              .await
              .get(&msg.business_id)
              .and_then(|handler| {
                let a = handler.upgrade();
                a
              })
            {
              handler.recv_msg(&msg);
            }
          }
        } else {
          tracing::error!("🔴Invalid message from websocket");
        }
      }
    });

    let mut sink_rx = self.sender.subscribe();
    tokio::spawn(async move {
      while let Ok(msg) = sink_rx.recv().await {
        tracing::trace!("[WS]: send message to web server");
        sink.send(msg.into()).await.unwrap();
      }
    });

    Ok(addr)
  }

  /// Return a [WSMessageHandler] that can be used to send messages to the websocket. Caller should
  /// keep the handler alive as long as it wants to receive messages from the websocket.
  pub async fn subscribe(&self, business_id: BusinessID) -> Result<Arc<WSMessageHandler>, WSError> {
    let handler = Arc::new(WSMessageHandler::new(
      business_id.clone(),
      self.sender.clone(),
    ));
    self
      .handlers
      .write()
      .await
      .insert(business_id, Arc::downgrade(&handler));
    Ok(handler)
  }

  async fn set_state(&self, state: ConnectState) {
    *self.state.lock().await = state;
  }
}

#[derive(Clone, Eq, PartialEq)]
pub enum ConnectState {
  Connecting,
  Connected,
  Disconnected,
}

impl ConnectState {
  #[allow(dead_code)]
  fn is_connecting(&self) -> bool {
    matches!(self, ConnectState::Connecting)
  }

  #[allow(dead_code)]
  fn is_connected(&self) -> bool {
    matches!(self, ConnectState::Connected)
  }

  #[allow(dead_code)]
  fn is_disconnected(&self) -> bool {
    matches!(self, ConnectState::Disconnected)
  }
}
