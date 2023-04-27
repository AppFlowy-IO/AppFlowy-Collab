use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;
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
    awareness: Arc<MutexCollabAwareness>,
    sink: Sink,
    stream: Stream,
  ) -> Self {
    Self::with_protocol(uid, object_id, awareness, sink, stream, CollabSyncProtocol)
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
    let cloned_oid = object_id.clone();
    let processing_loop: JoinHandle<Result<(), SyncError>> = spawn(async move {
      send_local_doc_state::<P, Sink, E>(
        uid,
        cloned_oid.clone(),
        &weak_sink,
        &weak_awareness,
        &protocol,
      )
      .await?;
      receive_remote_doc_changes(uid, cloned_oid, stream, weak_sink, weak_awareness, protocol)
        .await?;
      Ok(())
    });
    ClientSync {
      uid,
      object_id,
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
    let msg: CollabMessage = CollabClientMessage::new(uid, object_id, payload).into();
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
      Ok(data) => {
        match (weak_sink.upgrade(), weak_awareness.upgrade()) {
          (Some(mut sink), Some(awareness)) => {
            match process_message(
              uid,
              &object_id,
              &protocol,
              &awareness,
              &mut sink,
              data.into_payload(),
            )
            .await
            {
              Ok(()) => { /* continue */ },
              Err(e) => {
                return Err(e);
              },
            }
          },
          _ => {
            tracing::trace!("Doc is dropped. Stopping receive incoming doc changes.");
            return Ok(());
          }, // parent ConnHandler has been dropped
        }
      },
      Err(e) => return Err(e.into()),
    }
  }
  Ok(())
}

async fn process_message<P, E, Sink>(
  uid: i64,
  object_id: &str,
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

      let msg: CollabMessage =
        CollabClientMessage::new(uid, object_id.to_string(), reply.encode_v1()).into();
      if let Err(e) = sender.send(msg).await {
        tracing::error!("Failed to send reply to the client");
        return Err(e.into());
      } else {
        tracing::trace!("Reply to back to the client");
      }
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
