use crate::error::WSError;
use crate::msg::{BusinessID, WSMessage};
use crate::retry::ConnectAction;
use crate::WSObjectHandler;
use futures_util::{SinkExt, StreamExt};

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Weak};
use std::time::Duration;

use tokio::sync::broadcast::{channel, Receiver, Sender};
use tokio::sync::{Mutex, RwLock};
use tokio_retry::strategy::FixedInterval;
use tokio_retry::Retry;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::MaybeTlsStream;

pub struct WSClientConfig {
  /// specifies the number of messages that the channel can hold at any given
  /// time. It is used to set the initial size of the channel's internal buffer
  pub buffer_capacity: usize,
  /// specifies the number of seconds between each ping message
  pub ping_per_secs: u64,
  /// specifies the number of pings that the client will start reconnecting
  pub retry_connect_per_pings: u32,
}

impl Default for WSClientConfig {
  fn default() -> Self {
    Self {
      buffer_capacity: 1000,
      ping_per_secs: 8,
      retry_connect_per_pings: 10,
    }
  }
}

type HandlerByObjectId = HashMap<String, Weak<WSObjectHandler>>;

pub struct WSClient {
  addr: String,
  state: Arc<Mutex<ConnectStateNotify>>,
  sender: Sender<Message>,
  handlers: Arc<RwLock<HashMap<BusinessID, HandlerByObjectId>>>,
  ping: Arc<Mutex<ServerFixIntervalPing>>,
}

impl WSClient {
  pub fn new(addr: String, config: WSClientConfig) -> Self {
    let (sender, _) = channel(config.buffer_capacity);
    let state = Arc::new(Mutex::new(ConnectStateNotify::new()));
    let handlers = Arc::new(RwLock::new(HashMap::new()));
    let ping = Arc::new(Mutex::new(ServerFixIntervalPing::new(
      Duration::from_secs(config.ping_per_secs),
      state.clone(),
      sender.clone(),
      config.retry_connect_per_pings,
    )));
    WSClient {
      addr,
      state,
      sender,
      handlers,
      ping,
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
    let sender = self.sender.clone();
    self.ping.lock().await.run();
    // Receive messages from the websocket, and send them to the handlers.
    tokio::spawn(async move {
      while let Some(Ok(msg)) = stream.next().await {
        match msg {
          Message::Text(_) => {},
          Message::Binary(_) => {
            if let Ok(msg) = WSMessage::try_from(&msg) {
              if let Some(handlers) = weak_handlers.upgrade() {
                if let Some(handler) = handlers
                  .read()
                  .await
                  .get(&msg.business_id)
                  .and_then(|map| map.get(&msg.object_id))
                  .and_then(|handler| handler.upgrade())
                {
                  handler.recv_msg(&msg);
                }
              }
            } else {
              tracing::error!("🔴Invalid message from websocket");
            }
          },
          Message::Ping(_) => match sender.send(Message::Pong(vec![])) {
            Ok(_) => {},
            Err(e) => {
              tracing::error!("🔴Failed to send pong message to websocket: {:?}", e);
            },
          },
          Message::Pong(_) => {},
          Message::Close(_) => {},
          Message::Frame(_) => {},
        }
      }
    });

    let mut sink_rx = self.sender.subscribe();
    tokio::spawn(async move {
      while let Ok(msg) = sink_rx.recv().await {
        tracing::trace!("[WS Application]: send message to server");
        sink.send(msg).await.unwrap();
      }
    });

    Ok(addr)
  }

  /// Return a [WSObjectHandler] that can be used to send messages to the websocket. Caller should
  /// keep the handler alive as long as it wants to receive messages from the websocket.
  pub async fn subscribe(
    &self,
    business_id: BusinessID,
    object_id: String,
  ) -> Result<Arc<WSObjectHandler>, WSError> {
    let handler = Arc::new(WSObjectHandler::new(
      business_id,
      object_id.clone(),
      self.sender.clone(),
    ));
    self
      .handlers
      .write()
      .await
      .entry(business_id)
      .or_insert_with(HashMap::new)
      .insert(object_id, Arc::downgrade(&handler));
    Ok(handler)
  }

  pub async fn subscribe_connect_state(&self) -> Receiver<ConnectState> {
    self.state.lock().await.subscribe()
  }

  async fn set_state(&self, state: ConnectState) {
    self.state.lock().await.set_state(state);
  }
}

struct ServerFixIntervalPing {
  duration: Duration,
  sender: Option<Sender<Message>>,
  #[allow(dead_code)]
  stop_tx: tokio::sync::mpsc::Sender<()>,
  stop_rx: Option<tokio::sync::mpsc::Receiver<()>>,
  state: Arc<Mutex<ConnectStateNotify>>,
  ping_count: Arc<Mutex<u32>>,
  retry_connect_per_pings: u32,
}

impl ServerFixIntervalPing {
  fn new(
    duration: Duration,
    state: Arc<Mutex<ConnectStateNotify>>,
    sender: Sender<Message>,
    retry_connect_per_pings: u32,
  ) -> Self {
    let (tx, rx) = tokio::sync::mpsc::channel(1000);
    Self {
      duration,
      stop_tx: tx,
      stop_rx: Some(rx),
      state,
      sender: Some(sender),
      ping_count: Arc::new(Mutex::new(0)),
      retry_connect_per_pings,
    }
  }

  fn run(&mut self) {
    let mut stop_rx = self.stop_rx.take().expect("Only take once");
    let mut interval = tokio::time::interval(self.duration);
    let sender = self.sender.take().expect("Only take once");
    let mut receiver = sender.subscribe();
    let weak_ping_count = Arc::downgrade(&self.ping_count);
    let weak_state = Arc::downgrade(&self.state);
    let reconnect_per_ping = self.retry_connect_per_pings;
    tokio::spawn(async move {
      loop {
        tokio::select! {
          _ = interval.tick() => {
            // Send the ping
            tracing::trace!("🟢Send ping to server");
            let _ = sender.send(Message::Ping(vec![]));
            if let Some(ping_count) = weak_ping_count.upgrade() {
              let mut lock = ping_count.lock().await;
              // After ten ping were sent, mark the connection as disconnected
              if *lock >= reconnect_per_ping {
                if let Some(state) =weak_state.upgrade() {
                  state.lock().await.set_state(ConnectState::Disconnected);
                }
              } else {
                *lock +=1;
              }
            }
          },
          msg = receiver.recv() => {
            if let Ok(Message::Pong(_)) = msg {
              tracing::trace!("🟢Receive pong from server");
              if let Some(ping_count) = weak_ping_count.upgrade() {
                let mut lock = ping_count.lock().await;
                *lock = 0;

                if let Some(state) =weak_state.upgrade() {
                  state.lock().await.set_state(ConnectState::Connected);
                }
              }
            }
          },
          _ = stop_rx.recv() => {
            break;
          }
        }
      }
    });
  }
}

pub struct ConnectStateNotify {
  state: ConnectState,
  sender: Sender<ConnectState>,
}

impl ConnectStateNotify {
  fn new() -> Self {
    let (sender, _) = channel(100);
    Self {
      state: ConnectState::Disconnected,
      sender,
    }
  }

  fn set_state(&mut self, state: ConnectState) {
    if self.state != state {
      tracing::trace!("[WSClient]: connect state changed to {:?}", state);
      self.state = state.clone();
      let _ = self.sender.send(state);
    }
  }

  fn subscribe(&self) -> Receiver<ConnectState> {
    self.sender.subscribe()
  }
}

#[derive(Clone, Eq, PartialEq, Debug)]
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
