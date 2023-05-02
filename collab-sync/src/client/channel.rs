use std::pin::Pin;
use std::task::{Context, Poll};

use futures_util::{Sink, Stream};
use tokio::sync::mpsc::error::TryRecvError;
use tokio::sync::mpsc::{error::SendError, UnboundedReceiver, UnboundedSender};

pub trait CollabConnect<Item>: Sink<Item> + Stream {}

struct TokioUnboundedSink<T> {
  tx: UnboundedSender<T>,
}

impl<T> TokioUnboundedSink<T> {
  pub fn new(tx: UnboundedSender<T>) -> Self {
    Self { tx }
  }
}

impl<T> Sink<T> for TokioUnboundedSink<T> {
  type Error = SendError<T>;

  fn poll_ready(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
    // An unbounded channel can always accept messages without blocking, so we always return Ready.
    Poll::Ready(Ok(()))
  }

  fn start_send(self: Pin<&mut Self>, item: T) -> Result<(), Self::Error> {
    self.tx.send(item).map_err(|e| SendError(e.0))
  }

  fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
    // There is no buffering in an unbounded channel, so we always return Ready.
    Poll::Ready(Ok(()))
  }

  fn poll_close(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
    // An unbounded channel is closed by dropping the sender, so we don't need to do anything here.
    Poll::Ready(Ok(()))
  }
}

struct TokioUnboundedStream<T> {
  rx: UnboundedReceiver<T>,
}

impl<T> TokioUnboundedStream<T> {
  pub fn new(rx: UnboundedReceiver<T>) -> Self {
    Self { rx }
  }
}

impl<T> Stream for TokioUnboundedStream<T> {
  type Item = T;

  fn poll_next(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
    match self.rx.try_recv() {
      Ok(item) => Poll::Ready(Some(item)),
      Err(TryRecvError::Empty) => Poll::Pending,
      Err(TryRecvError::Disconnected) => Poll::Ready(None),
    }
  }
}
