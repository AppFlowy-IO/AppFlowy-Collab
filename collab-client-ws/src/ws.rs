use crate::error::WSError;
use crate::msg::{TargetID, WSMessage};
use crate::retry::ConnectAction;
use crate::WSMessageHandler;
use futures_util::{SinkExt, StreamExt};

use std::collections::HashMap;
use std::sync::{Arc, Weak};
use std::time::Duration;
use tokio::sync::broadcast::{channel, Sender};
use tokio::sync::{Mutex, RwLock};
use tokio_retry::strategy::FixedInterval;
use tokio_retry::Retry;

pub struct WSClient {
  addr: String,
  state: Mutex<ConnectState>,
  sender: Sender<WSMessage>,
  handlers: Arc<RwLock<HashMap<TargetID, Weak<WSMessageHandler>>>>,
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

  pub async fn connect(&self) -> Result<(), WSError> {
    self.set_state(ConnectState::Connecting).await;
    let retry_strategy = FixedInterval::new(Duration::from_secs(2)).take(3);
    let action = ConnectAction::new(self.addr.clone());
    let stream = Retry::spawn(retry_strategy, action).await?;
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
              .get(&msg.id)
              .and_then(|handler| handler.upgrade())
            {
              handler.recv_msg(&msg);
            }
          }
        }
      }
    });

    let mut sink_rx = self.sender.subscribe();
    tokio::spawn(async move {
      while let Ok(msg) = sink_rx.recv().await {
        sink.send(msg.into()).await.unwrap();
      }
    });

    Ok(())
  }

  pub async fn subscribe_with_sender(
    &self,
    target_id: TargetID,
  ) -> Result<Arc<WSMessageHandler>, WSError> {
    let handler = Arc::new(WSMessageHandler::new(
      target_id.clone(),
      self.sender.clone(),
    ));
    self
      .handlers
      .write()
      .await
      .insert(target_id, Arc::downgrade(&handler));
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
