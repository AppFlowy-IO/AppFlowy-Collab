use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use collab::core::collab::MutexCollab;
use collab_sync::server::{
  CollabBroadcast, CollabGroup, CollabIDGen, CollabId, CollabMsgCodec, CollabSink, CollabStream,
  NonZeroNodeId,
};
use dashmap::DashMap;
use parking_lot::Mutex;
use serde_json::Value;
use tempfile::TempDir;

use collab::core::origin::{CollabClient, CollabOrigin};
use collab_persistence::kv::rocks_kv::RocksCollabDB;

use collab::preclude::Collab;
use collab_plugins::disk_plugin::rocksdb_server::RocksdbServerDiskPlugin;
use tokio::net::TcpListener;

use crate::setup_log;

pub struct TestServer {
  pub db: Arc<RocksCollabDB>,
  pub collab_id_gen: Arc<Mutex<CollabIDGen>>,
  pub groups: Arc<DashMap<String, CollabGroup>>,
  pub address: SocketAddr,
  pub port: u16,
  #[allow(dead_code)]
  cleaner: Cleaner,
}

impl TestServer {
  pub fn get_doc_json(&self, object_id: &str) -> Value {
    self
      .groups
      .entry(object_id.to_string())
      .or_insert_with(|| {
        let collab_id = self.collab_id_gen.lock().next_id();
        make_collab_group(collab_id, &object_id, self.db.clone())
      })
      .collab
      .to_json_value()
  }

  pub fn mut_groups(&self, object_id: &str, f: impl FnOnce(&Collab)) {
    self
      .groups
      .entry(object_id.to_string())
      .or_insert_with(|| {
        let collab_id = self.collab_id_gen.lock().next_id();
        make_collab_group(collab_id, &object_id, self.db.clone())
      })
      .get_mut_collab(f);
  }
}

pub async fn spawn_server(object_id: &str) -> std::io::Result<TestServer> {
  let tempdir = TempDir::new().unwrap();
  let path = tempdir.into_path();
  let db = Arc::new(RocksCollabDB::open(path.clone()).unwrap());
  let cleaner = Cleaner::new(path);

  setup_log();
  let collab_id_gen = Arc::new(Mutex::new(CollabIDGen::new(NonZeroNodeId(1))));
  let address = SocketAddr::from(([127, 0, 0, 1], 0));
  let listener = TcpListener::bind(address).await?;
  let port = listener.local_addr()?.port(); // Get the actual port number
  let groups = Arc::new(DashMap::new());
  let object_id = object_id.to_string();
  let cloned_db = db.clone();

  let weak_groups = Arc::downgrade(&groups);
  let weak_collab_id_gen = Arc::downgrade(&collab_id_gen);
  tokio::spawn(async move {
    while let Ok((stream, client_addr)) = listener.accept().await {
      let (reader, writer) = stream.into_split();
      let stream = CollabStream::new(reader, CollabMsgCodec::default());
      let sink = CollabSink::new(writer, CollabMsgCodec::default());
      let client = CollabClient::new(client_addr.port() as i64, &client_addr.to_string());
      let groups = weak_groups.upgrade().unwrap();
      let collab_id_gen = weak_collab_id_gen.upgrade().unwrap();

      let sub = groups
        .entry(object_id.clone())
        .or_insert_with(|| {
          let collab_id = collab_id_gen.lock().next_id();
          make_collab_group(collab_id, &object_id, cloned_db.clone())
        })
        .broadcast
        .subscribe(
          CollabOrigin::Client(client.clone()),
          Arc::new(tokio::sync::Mutex::new(sink)),
          stream,
        );

      groups
        .get_mut(&object_id)
        .unwrap()
        .subscribers
        .insert(CollabOrigin::Client(client), sub);
    }
  });

  Ok(TestServer {
    db,
    collab_id_gen,
    address: SocketAddr::from(([127, 0, 0, 1], port)),
    port,
    groups,
    cleaner,
  })
}

pub fn make_collab_group(
  collab_id: CollabId,
  object_id: &str,
  db: Arc<RocksCollabDB>,
) -> CollabGroup {
  let collab = MutexCollab::new(CollabOrigin::Empty, object_id, vec![]);
  let plugin = RocksdbServerDiskPlugin::new(collab_id, db).unwrap();
  collab.lock().add_plugin(Arc::new(plugin));

  let broadcast = CollabBroadcast::new(object_id, collab.clone(), 10);
  CollabGroup {
    collab,
    broadcast,
    subscribers: Default::default(),
  }
}

struct Cleaner(PathBuf);

impl Cleaner {
  fn new(dir: PathBuf) -> Self {
    Cleaner(dir)
  }

  fn cleanup(dir: &PathBuf) {
    let _ = std::fs::remove_dir_all(dir);
  }
}

impl Drop for Cleaner {
  fn drop(&mut self) {
    Self::cleanup(&self.0)
  }
}
