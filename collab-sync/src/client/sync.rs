use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Weak};
use std::task::{Context, Poll};

use collab::core::collab_awareness::MutexCollabAwareness;
use futures_util::sink::SinkExt;
use futures_util::StreamExt;
use lib0::decoding::Cursor;
use tokio::spawn;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use y_sync::sync::{MessageReader, Protocol};
use yrs::updates::decoder::DecoderV1;
use yrs::updates::encoder::{Encode, Encoder, EncoderV1};

use crate::error::SyncError;
use crate::message::{CollabClientMessage, CollabMessage};
use crate::protocol::{handle_msg, CollabSyncProtocol};

pub struct ClientSync<Sink, Stream> {
  #[allow(dead_code)]
  uid: i64,
  #[allow(dead_code)]
  object_id: String,
  msg_id_counter: Arc<AtomicU32>,
  processing_loop: JoinHandle<Result<(), SyncError>>,
  awareness: Arc<MutexCollabAwareness>,
  inbox: Arc<Mutex<Sink>>,
  _stream: PhantomData<Stream>,
}

impl<Sink, Stream, E> ClientSync<Sink, Stream>
where
  E: Into<SyncError> + Send + Sync,
  Sink: SinkExt<CollabMessage, Error = E> + Send + Sync + Unpin + 'static,
{
  pub async fn send(&self, msg: CollabMessage) -> Result<(), SyncError> {
    let mut inbox = self.inbox.lock().await;
    match inbox.send(msg).await {
      Ok(_) => Ok(()),
      Err(err) => Err(err.into()),
    }
  }

  pub async fn close(self) -> Result<(), E> {
    let mut inbox = self.inbox.lock().await;
    inbox.close().await
  }

  pub fn sink(&self) -> Weak<Mutex<Sink>> {
    Arc::downgrade(&self.inbox)
  }
}

impl<Sink, Stream, E> ClientSync<Sink, Stream>
where
  E: Into<SyncError> + Send + Sync,
  Sink: SinkExt<CollabMessage, Error = E> + Send + Sync + Unpin + 'static,
  Stream: StreamExt<Item = Result<CollabMessage, E>> + Send + Sync + Unpin + 'static,
{
  /// Wraps incoming [WebSocket] connection and supplied [Awareness] accessor into a new
  /// connection handler capable of exchanging Yrs/Yjs messages.
  pub fn new(
    uid: i64,
    object_id: &str,
    msg_id_counter: Arc<AtomicU32>,
    awareness: Arc<MutexCollabAwareness>,
    sink: Sink,
    stream: Stream,
  ) -> Self {
    Self::with_protocol(
      uid,
      object_id,
      msg_id_counter,
      awareness,
      sink,
      stream,
      CollabSyncProtocol,
    )
  }

  /// Returns an underlying [Awareness] structure, that contains client state of that connection.
  pub fn awareness(&self) -> &Arc<MutexCollabAwareness> {
    &self.awareness
  }

  /// Wraps incoming [WebSocket] connection and supplied [Awareness] accessor into a new
  /// connection handler capable of exchanging Yrs/Yjs messages.
  pub fn with_protocol<P>(
    uid: i64,
    object_id: &str,
    msg_id_counter: Arc<AtomicU32>,
    awareness: Arc<MutexCollabAwareness>,
    sink: Sink,
    stream: Stream,
    protocol: P,
  ) -> Self
  where
    P: Protocol + Send + Sync + 'static,
  {
    let object_id = object_id.to_string();
    let sink = Arc::new(Mutex::new(sink));
    let inbox = sink.clone();
    let weak_sink = Arc::downgrade(&sink);
    let weak_awareness = Arc::downgrade(&awareness);
    let weak_msg_id_counter = Arc::downgrade(&msg_id_counter);
    let cloned_oid = object_id.clone();
    let processing_loop: JoinHandle<Result<(), SyncError>> = spawn(async move {
      // Send the initial document state when the client connects
      send_local_doc_state::<P, Sink, E>(
        uid,
        cloned_oid.clone(),
        weak_msg_id_counter.clone(),
        &weak_sink,
        &weak_awareness,
        &protocol,
      )
      .await?;

      // Spawn the stream that continuously reads messages from the client
      receive_remote_doc_changes(
        uid,
        cloned_oid,
        weak_msg_id_counter,
        stream,
        weak_sink,
        weak_awareness,
        protocol,
      )
      .await?;

      Ok(())
    });
    ClientSync {
      uid,
      object_id,
      msg_id_counter,
      processing_loop,
      awareness,
      inbox,
      _stream: PhantomData::default(),
    }
  }
}

/// To be called whenever a new connection has been accepted
async fn send_local_doc_state<P, Sink, E>(
  uid: i64,
  object_id: String,
  msg_id_counter: Weak<AtomicU32>,
  weak_sink: &Weak<Mutex<Sink>>,
  weak_awareness: &Weak<MutexCollabAwareness>,
  protocol: &P,
) -> Result<(), SyncError>
where
  P: Protocol,
  E: Into<SyncError> + Send + Sync,
  Sink: SinkExt<CollabMessage, Error = E> + Send + Sync + Unpin + 'static,
{
  let payload = {
    let awareness = weak_awareness.upgrade().unwrap();
    let mut encoder = EncoderV1::new();
    let awareness = awareness.lock();
    protocol.start(&awareness, &mut encoder)?;
    encoder.to_vec()
  };

  if !payload.is_empty() {
    let msg_id = msg_id_counter
      .upgrade()
      .unwrap()
      .fetch_add(1, Ordering::SeqCst);
    let msg: CollabMessage = CollabClientMessage::new(uid, object_id, msg_id, payload).into();
    if let Some(sink) = weak_sink.upgrade() {
      let mut s = sink.lock().await;
      if let Err(e) = s.send(msg).await {
        return Err(e.into());
      }
    } else {
      return Ok(());
    }
  }
  Ok(())
}

async fn receive_remote_doc_changes<E, Sink, Stream, P>(
  uid: i64,
  object_id: String,
  msg_id_counter: Weak<AtomicU32>,
  mut stream: Stream,
  weak_sink: Weak<Mutex<Sink>>,
  weak_awareness: Weak<MutexCollabAwareness>,
  protocol: P,
) -> Result<(), SyncError>
where
  P: Protocol,
  E: Into<SyncError> + Send + Sync,
  Sink: SinkExt<CollabMessage, Error = E> + Send + Sync + Unpin + 'static,
  Stream: StreamExt<Item = Result<CollabMessage, E>> + Send + Sync + Unpin + 'static,
{
  while let Some(input) = stream.next().await {
    match input {
      Ok(msg) => {
        match (
          weak_sink.upgrade(),
          weak_awareness.upgrade(),
          msg_id_counter.upgrade(),
        ) {
          (Some(mut sink), Some(awareness), Some(msg_id_counter)) => {
            if let Err(e) = process_message(
              uid,
              &object_id,
              &msg_id_counter,
              &protocol,
              &awareness,
              &mut sink,
              msg.into_payload(),
            )
            .await
            {
              return Err(e);
            }
          },
          _ => {
            tracing::trace!("ClientSync is dropped. Stopping receive incoming changes.");
            return Ok(());
          },
        }
      },
      Err(e) => {
        // Stop receiving messages if the client has disconnected
        return Err(e.into());
      },
    }
  }
  Ok(())
}

async fn process_message<P, E, Sink>(
  uid: i64,
  object_id: &str,
  msg_id_counter: &Arc<AtomicU32>,
  protocol: &P,
  awareness: &Arc<MutexCollabAwareness>,
  sink: &mut Arc<Mutex<Sink>>,
  input: Vec<u8>,
) -> Result<(), SyncError>
where
  P: Protocol,
  E: Into<SyncError> + Send + Sync,
  Sink: SinkExt<CollabMessage, Error = E> + Send + Sync + Unpin + 'static,
{
  let mut decoder = DecoderV1::new(Cursor::new(&input));
  let reader = MessageReader::new(&mut decoder);
  for msg in reader {
    let msg = msg?;
    if let Some(reply) = handle_msg(protocol, awareness, msg).await? {
      let mut sender = sink.lock().await;

      let msg_id = msg_id_counter.fetch_add(1, Ordering::SeqCst);
      let payload = reply.encode_v1();
      let msg = CollabClientMessage::new(uid, object_id.to_string(), msg_id, payload);

      sender.send(msg.into()).await?;
    }
  }
  Ok(())
}

impl<Sink, Stream> Unpin for ClientSync<Sink, Stream> {}

impl<Sink, Stream> Future for ClientSync<Sink, Stream> {
  type Output = Result<(), SyncError>;

  fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
    match Pin::new(&mut self.processing_loop).poll(cx) {
      Poll::Pending => Poll::Pending,
      Poll::Ready(Err(e)) => Poll::Ready(Err(e.into())),
      Poll::Ready(Ok(r)) => Poll::Ready(r),
    }
  }
}
