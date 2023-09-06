use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering::SeqCst;
use std::sync::Arc;

use crate::util::{CollabSink, CollabStream};
use collab_plugins::sync_plugin::client::SyncError;
use collab_sync_protocol::CollabMessage;
use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio::task::JoinHandle;

pub struct TestStream {
  #[allow(dead_code)]
  runner: JoinHandle<()>,
  is_conn: Arc<AtomicBool>,
}

impl TestStream {
  pub fn new(
    mut inner: CollabStream,
    sender: UnboundedSender<Result<CollabMessage, SyncError>>,
  ) -> Self {
    let is_conn = Arc::new(AtomicBool::new(true));
    let cloned_is_conn = is_conn.clone();
    let runner = tokio::spawn(async move {
      while let Some(msg) = inner.next().await {
        if cloned_is_conn.load(SeqCst) {
          sender.send(msg).unwrap();
        }
      }
    });

    Self { runner, is_conn }
  }

  pub fn disconnect(&self) {
    self.is_conn.store(false, SeqCst);
  }

  pub fn connect(&self) {
    self.is_conn.store(true, SeqCst);
  }
}

pub struct TestSink {
  #[allow(dead_code)]
  runner: JoinHandle<()>,
  is_conn: Arc<AtomicBool>,
}

impl TestSink {
  pub fn new(mut inner: CollabSink, mut recv: UnboundedReceiver<CollabMessage>) -> Self {
    let is_conn = Arc::new(AtomicBool::new(true));
    let cloned_is_conn = is_conn.clone();
    let runner = tokio::spawn(async move {
      while let Some(msg) = recv.recv().await {
        if cloned_is_conn.load(SeqCst) {
          inner.send(msg).await.unwrap();
        }
      }
    });
    Self { runner, is_conn }
  }
  pub fn disconnect(&self) {
    self.is_conn.store(false, SeqCst);
  }

  pub fn connect(&self) {
    self.is_conn.store(true, SeqCst);
  }
}
