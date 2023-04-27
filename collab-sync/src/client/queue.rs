use futures_util::SinkExt;
use crate::error::SyncError;
use crate::message::CollabMessage;

pub struct SyncQueue<Sink> {}

impl<E, Sink> SyncQueue<Sink> where
    E: Into<SyncError> + Send + Sync,
    Sink: SinkExt<CollabMessage, Error=E> + Send + Sync + Unpin + 'static, {}
