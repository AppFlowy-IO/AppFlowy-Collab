use bytes::{Bytes, BytesMut};
use collab_sync::server::{BroadcastGroup, Subscription};
use dashmap::DashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::str::FromStr;
use std::sync::Arc;
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::TcpListener;
use tokio_util::codec::{Decoder, Encoder, FramedRead, FramedWrite, LengthDelimitedCodec};
use y_sync::sync::Error;

pub struct TestServer {
  pub groups: Arc<DashMap<String, Group>>,
  pub address: String,
  pub port: u16,
}

#[derive(Debug, Default)]
struct YrsCodec(LengthDelimitedCodec);

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

type WrappedStream = FramedRead<OwnedReadHalf, YrsCodec>;
type WrappedSink = FramedWrite<OwnedWriteHalf, YrsCodec>;

pub async fn spawn_server() -> std::io::Result<TestServer> {
  let addr = SocketAddr::from(([127, 0, 0, 1], 0));
  let listener = TcpListener::bind(addr).await?;
  let port = listener.local_addr()?.port(); // Get the actual port number
  let groups = Arc::new(DashMap::new());

  let weak_groups = Arc::downgrade(&groups);
  tokio::spawn(async move {
    let mut subscribers = Vec::new();
    while let Ok((stream, _)) = listener.accept().await {
      let (reader, writer) = stream.into_split();
      let stream = WrappedStream::new(reader, YrsCodec::default());
      let sink = WrappedSink::new(writer, YrsCodec::default());

      // let sub = bcast.subscribe(Arc::new(Mutex::new(sink)), stream);
      // subscribers.push(sub);
    }
  });

  Ok(TestServer {
    address: "127.0.0.1".to_string(),
    port,
    groups,
  })
}

struct Group {
  broadcast: BroadcastGroup,
  subscriptions: Vec<Subscription>,
}
