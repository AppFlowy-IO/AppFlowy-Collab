use crate::cloud_storage::error::SyncError;
use futures::Sink;
use std::fmt::Debug;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::sync::mpsc::UnboundedSender;
use tokio_stream::Stream;

#[allow(dead_code)]
pub trait CollabConnect<Item>: Sink<Item> + Stream {}

pub struct TokioUnboundedSink<T>(pub UnboundedSender<T>);

impl<T> Sink<T> for TokioUnboundedSink<T>
where
  T: Send + Sync + 'static + Debug,
{
  type Error = SyncError;

  fn poll_ready(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
    // An unbounded channel can always accept messages without blocking, so we always return Ready.
    Poll::Ready(Ok(()))
  }

  fn start_send(self: Pin<&mut Self>, item: T) -> Result<(), Self::Error> {
    let _ = self.0.send(item);
    Ok(())
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
