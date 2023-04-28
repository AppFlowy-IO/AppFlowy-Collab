use crate::error::SyncError;
use crate::message::CollabMessage;
use futures_util::{Sink, Stream};
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

impl Sink<CollabMessage> for UnboundedSender<CollabMessage> {
  type Error = SyncError;

  fn poll_ready(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
    Poll::Ready(Ok(()))
  }

  fn start_send(self: Pin<&mut Self>, item: CollabMessage) -> Result<(), Self::Error> {
    self
      .send(item)
      .map_err(|e| SyncError::Internal(Box::new(e)))?;
    Ok(())
  }

  fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
    Poll::Ready(Ok(()))
  }

  fn poll_close(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
    Poll::Ready(Ok(()))
  }
}

pub struct TokioUnboundedReceiver(pub UnboundedReceiver<CollabMessage>);

impl Stream for TokioUnboundedReceiver {
  type Item = Result<CollabMessage, SyncError>;

  fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
    match Pin::new(&mut self.0).poll_recv(cx) {
      Poll::Ready(Some(item)) => Poll::Ready(Some(Ok(item))),
      Poll::Ready(None) => Poll::Ready(None),
      Poll::Pending => Poll::Pending,
    }
  }
}
