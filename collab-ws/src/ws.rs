use crate::error::WSError;
use serde::Serialize;
use tokio_retry::strategy::{ExponentialBackoff, FixedInterval};
use tokio_retry::Retry;
use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;
use tokio_tungstenite::tungstenite::protocol::CloseFrame;
use tokio_tungstenite::tungstenite::Message;

pub struct WSConnect {
  state: ConnectState,
}

impl WSConnect {
  pub fn new() -> Self {
    WSConnect {
      state: ConnectState::Disconnected,
    }
  }

  pub async fn start(&self) {
    let retry_strategy = FixedInterval::from_millis(10).take(3);

    // let result = Retry::spawn(retry_strategy, action).await?;
  }

  pub fn send_msg<T: Serialize>(&self, msg: T) -> Result<(), WSError> {
    todo!()
  }

  pub fn disconnect(&self, reason: &str) -> Result<(), WSError> {
    let frame = CloseFrame {
      code: CloseCode::Normal,
      reason: reason.to_owned().into(),
    };
    let msg = Message::Close(Some(frame));
    todo!()
  }
}

#[derive(Clone, Eq, PartialEq)]
pub enum ConnectState {
  Connecting,
  Connected,
  Disconnected,
}

impl ConnectState {
  fn is_connecting(&self) -> bool {
    matches!(self, ConnectState::Connecting)
  }

  fn is_connected(&self) -> bool {
    matches!(self, ConnectState::Connected)
  }

  fn is_disconnected(&self) -> bool {
    matches!(self, ConnectState::Disconnected)
  }
}

struct ConnectAction {
  addr: String,
}
