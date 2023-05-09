use crate::error::WSError;
use crate::{HandlerID, WSMessage};
use futures_util::Sink;
use std::fmt::Debug;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::sync::broadcast::{channel, Sender};
use tokio_stream::wrappers::BroadcastStream;

pub struct WSMessageHandler {
  target_id: HandlerID,
  sender: Sender<WSMessage>,
  receiver: Sender<WSMessage>,
}

impl WSMessageHandler {
  pub fn new(target_id: HandlerID, sender: Sender<WSMessage>) -> Self {
    let (receiver, _) = channel(1000);
    Self {
      target_id,
      sender,
      receiver,
    }
  }

  pub fn target_id(&self) -> &str {
    &self.target_id
  }

  pub(crate) fn recv_msg(&self, msg: &WSMessage) {
    let _ = self.receiver.send(msg.clone());
  }

  pub fn sink<T>(&self) -> BroadcastSink<T>
  where
    T: Into<WSMessage> + Send + Sync + 'static + Clone,
  {
    let (tx, mut rx) = channel::<T>(1000);
    let cloned_sender = self.sender.clone();
    tokio::spawn(async move {
      while let Ok(msg) = rx.recv().await {
        let _ = cloned_sender.send(msg.into());
      }
    });
    BroadcastSink::new(tx)
  }

  pub fn stream(&self) -> BroadcastStream<WSMessage> {
    BroadcastStream::new(self.receiver.subscribe())
  }
}

pub struct BroadcastSink<T>(pub Sender<T>);

impl<T> BroadcastSink<T> {
  pub fn new(tx: Sender<T>) -> Self {
    Self(tx)
  }
}

impl<T> Sink<T> for BroadcastSink<T>
where
  T: Send + Sync + 'static + Debug,
{
  type Error = WSError;

  fn poll_ready(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
    Poll::Ready(Ok(()))
  }

  fn start_send(self: Pin<&mut Self>, item: T) -> Result<(), Self::Error> {
    let _ = self.0.send(item);
    Ok(())
  }

  fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
    Poll::Ready(Ok(()))
  }

  fn poll_close(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
    Poll::Ready(Ok(()))
  }
}
