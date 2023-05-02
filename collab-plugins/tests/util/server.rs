use std::net::SocketAddr;
use std::sync::Arc;

use collab::core::collab::CollabOrigin;
use collab::core::collab_awareness::MutexCollabAwareness;
use collab::preclude::Collab;

use collab_sync::msg_codec::{CollabMsgCodec, CollabSink, CollabStream};
use collab_sync::server::{BroadcastGroup, Subscription};
use dashmap::DashMap;
use serde_json::Value;

use tokio::net::TcpListener;
use tokio::sync::Mutex;

use crate::setup_log;

pub struct TestServer {
  pub groups: Arc<DashMap<String, CollabGroup>>,
  pub address: SocketAddr,
  pub port: u16,
}

impl TestServer {
  pub fn get_doc_json(&self, object_id: &str) -> Value {
    self
      .groups
      .get(object_id)
      .unwrap()
      .awareness
      .lock()
      .collab
      .to_json_value()
  }
}

pub async fn spawn_server(uid: i64, object_id: &str) -> std::io::Result<TestServer> {
  let group = make_test_collab_group(uid, object_id).await;
  spawn_server_with_data(group).await
}

pub async fn spawn_server_with_data(group: CollabGroup) -> std::io::Result<TestServer> {
  setup_log();

  let address = SocketAddr::from(([127, 0, 0, 1], 0));
  let listener = TcpListener::bind(address).await?;
  let port = listener.local_addr()?.port(); // Get the actual port number
  let groups = Arc::new(DashMap::new());

  let object_id = group.awareness.lock().collab.object_id.clone();
  groups.insert(object_id.clone(), group);

  let weak_groups = Arc::downgrade(&groups);
  tokio::spawn(async move {
    while let Ok((stream, _)) = listener.accept().await {
      let (reader, writer) = stream.into_split();
      let stream = CollabStream::new(reader, CollabMsgCodec::default());
      let sink = CollabSink::new(writer, CollabMsgCodec::default());

      // Hardcode doc_id 1 for test
      let groups = weak_groups.upgrade().unwrap();
      let sub = groups
        .get(&object_id)
        .unwrap()
        .broadcast
        .subscribe(Arc::new(Mutex::new(sink)), stream);
      groups.get_mut(&object_id).unwrap().subscriptions.push(sub);
    }
  });

  Ok(TestServer {
    address: SocketAddr::from(([127, 0, 0, 1], port)),
    port,
    groups,
  })
}

pub async fn make_test_collab_group(uid: i64, object_id: &str) -> CollabGroup {
  let awareness = MutexCollabAwareness::new(CollabOrigin::new(uid, "remote"), object_id, vec![]);
  let broadcast = BroadcastGroup::new(object_id, awareness.clone(), 10).await;
  CollabGroup {
    awareness,
    broadcast,
    subscriptions: vec![],
  }
}

pub struct CollabGroup {
  pub awareness: MutexCollabAwareness,
  pub broadcast: BroadcastGroup,
  subscriptions: Vec<Subscription>,
}

impl CollabGroup {
  pub fn mut_collab<F>(&self, f: F)
  where
    F: FnOnce(&Collab),
  {
    let awareness = self.awareness.lock();
    f(&awareness.collab);
  }
}
