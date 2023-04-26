use collab_sync::client::Connection;
use collab_sync::error::SyncError;
use futures_util::{SinkExt, StreamExt};
use std::sync::Arc;
use tokio::sync::RwLock;
use y_sync::awareness::Awareness;

pub struct SyncPlugin<Sink, Stream> {
  awareness: Arc<RwLock<Awareness>>,
  conn: Connection<Sink, Stream>,
}

impl<Sink, Stream> SyncPlugin<Sink, Stream> {
  pub fn new<E>(awareness: Arc<RwLock<Awareness>>, sink: Sink, stream: Stream) -> Self
  where
    E: Into<SyncError> + Send + Sync,
    Sink: SinkExt<Vec<u8>, Error = E> + Send + Sync + Unpin + 'static,
    Stream: StreamExt<Item = Result<Vec<u8>, E>> + Send + Sync + Unpin + 'static,
  {
    let conn = Connection::new(awareness.clone(), sink, stream);
    Self { awareness, conn }
  }
}
