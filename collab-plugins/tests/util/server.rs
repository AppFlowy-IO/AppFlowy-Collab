use std::net::SocketAddr;
use std::sync::Arc;

use bytes::{Bytes, BytesMut};
use collab::core::collab_awareness::MutexCollabAwareness;

use collab_sync::server::{BroadcastGroup, Subscription};
use dashmap::DashMap;
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::TcpListener;
use tokio::sync::Mutex;
use tokio_util::codec::{Decoder, Encoder, FramedRead, FramedWrite, LengthDelimitedCodec};

use y_sync::sync::Error;

use crate::setup_log;

pub struct TestServer {
  pub groups: Arc<DashMap<String, Group>>,
  pub address: SocketAddr,
  pub port: u16,
}

#[derive(Debug, Default)]
pub struct YrsCodec(LengthDelimitedCodec);

impl Encoder<Vec<u8>> for YrsCodec {
  type Error = Error;

  fn encode(&mut self, item: Vec<u8>, dst: &mut BytesMut) -> Result<(), Self::Error> {
    self.0.encode(Bytes::from(item), dst)?;
    Ok(())
  }
}

impl Decoder for YrsCodec {
  type Item = Vec<u8>;
  type Error = Error;

  fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
    if let Some(bytes) = self.0.decode(src)? {
      Ok(Some(bytes.freeze().to_vec()))
    } else {
      Ok(None)
    }
  }
}

pub type WrappedStream = FramedRead<OwnedReadHalf, YrsCodec>;
pub type WrappedSink = FramedWrite<OwnedWriteHalf, YrsCodec>;

pub async fn spawn_server() -> std::io::Result<TestServer> {
  setup_log();

  let address = SocketAddr::from(([127, 0, 0, 1], 0));
  let listener = TcpListener::bind(address).await?;
  let port = listener.local_addr()?.port(); // Get the actual port number
  let groups = Arc::new(DashMap::new());

  let (doc_id, group) = test_group().await;
  groups.insert(doc_id.clone(), group);

  let weak_groups = Arc::downgrade(&groups);
  tokio::spawn(async move {
    while let Ok((stream, _)) = listener.accept().await {
      let (reader, writer) = stream.into_split();
      let stream = WrappedStream::new(reader, YrsCodec::default());
      let sink = WrappedSink::new(writer, YrsCodec::default());

      // Hardcode doc_id 1 for test
      let groups = weak_groups.upgrade().unwrap();
      let sub = groups
        .get(&doc_id)
        .unwrap()
        .broadcast
        .subscribe(Arc::new(Mutex::new(sink)), stream);
      groups.get_mut(&doc_id).unwrap().subscriptions.push(sub);
    }
  });

  Ok(TestServer {
    address: SocketAddr::from(([127, 0, 0, 1], port)),
    port,
    groups,
  })
}

async fn test_group() -> (String, Group) {
  let doc_id = "1".to_string();
  let uid = 1;
  let awareness = MutexCollabAwareness::new(uid, &doc_id, vec![]);
  let broadcast = BroadcastGroup::new(awareness.clone(), 10).await;
  (
    doc_id,
    Group {
      awareness,
      broadcast,
      subscriptions: vec![],
    },
  )
}

pub struct Group {
  pub awareness: MutexCollabAwareness,
  broadcast: BroadcastGroup,
  subscriptions: Vec<Subscription>,
}
