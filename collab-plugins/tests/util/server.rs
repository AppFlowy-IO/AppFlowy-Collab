use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use collab::core::collab::MutexCollab;
use collab::core::origin::{CollabClient, CollabOrigin};
use collab::preclude::Collab;
use collab_persistence::kv::rocks_kv::RocksCollabDB;
use collab_persistence::kv::KVStore;
use collab_sync::server::{
  CollabBroadcast, CollabGroup, CollabIDGen, CollabId, CollabMsgCodec, CollabSink, CollabStream,
  NonZeroNodeId, COLLAB_ID_LEN,
};
use parking_lot::Mutex;
use serde_json::Value;
use tokio::net::TcpListener;

use collab_plugins::local_storage::rocksdb_server::RocksdbServerDiskPlugin;
use dashmap::DashMap;
use futures::executor::block_on;
use tempfile::TempDir;

use crate::setup_log;

pub struct TestServer {
  pub db_path: PathBuf,
  pub db: Arc<RocksCollabDB>,
  pub collab_id_gen: Arc<Mutex<CollabIDGen>>,
  pub groups: Arc<DashMap<String, CollabGroup>>,
  pub address: SocketAddr,
  pub port: u16,
  pub cleaner: Cleaner,
}

impl TestServer {
  pub fn get_doc_json(&self, object_id: &str) -> Value {
    self
      .groups
      .entry(object_id.to_string())
      .or_insert_with(|| {
        let collab_id = self.collab_id_from_object_id(object_id);
        block_on(make_collab_group(collab_id, object_id, self.db.clone()))
      })
      .collab
      .to_json_value()
  }

  pub fn mut_groups(&self, object_id: &str, f: impl FnOnce(&Collab)) {
    self
      .groups
      .entry(object_id.to_string())
      .or_insert_with(|| {
        let collab_id = self.collab_id_from_object_id(object_id);
        block_on(make_collab_group(collab_id, object_id, self.db.clone()))
      })
      .get_mut_collab(f);
  }

  pub fn collab_id_from_object_id(&self, object_id: &str) -> CollabId {
    let read_txn = self.db.read_txn();
    let value = read_txn.get(object_id).ok();

    match value {
      Some(Some(value)) => {
        let mut bytes = [0; COLLAB_ID_LEN];
        bytes[0..COLLAB_ID_LEN].copy_from_slice(value.as_ref());
        CollabId::from_be_bytes(bytes)
      },
      _ => self.collab_id_gen.lock().next_id(),
    }
  }
}

pub async fn spawn_server(object_id: &str) -> std::io::Result<TestServer> {
  let tempdir = TempDir::new().unwrap();
  let path = tempdir.into_path();
  let db = Arc::new(RocksCollabDB::open(path.clone()).unwrap());
  spawn_server_with_db(object_id, path, db).await
}

pub async fn spawn_server_with_db(
  object_id: &str,
  db_path: PathBuf,
  db: Arc<RocksCollabDB>,
) -> std::io::Result<TestServer> {
  let cleaner = Cleaner::new(db_path.clone());
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
          // Map the object_id to the collab_id
          cloned_db
            .with_write_txn(|w_txn| {
              w_txn.insert(object_id.clone(), collab_id.to_be_bytes())?;
              Ok(())
            })
            .unwrap();

          block_on(make_collab_group(collab_id, &object_id, cloned_db.clone()))
        })
        .broadcast
        .subscribe(CollabOrigin::Client(client.clone()), sink, stream);

      groups
        .get_mut(&object_id)
        .unwrap()
        .subscribers
        .insert(CollabOrigin::Client(client), sub);
    }
  });

  Ok(TestServer {
    db_path,
    db,
    collab_id_gen,
    address: SocketAddr::from(([127, 0, 0, 1], port)),
    port,
    groups,
    cleaner,
  })
}

pub async fn make_collab_group(
  collab_id: CollabId,
  object_id: &str,
  db: Arc<RocksCollabDB>,
) -> CollabGroup {
  let collab = MutexCollab::new(CollabOrigin::Server, object_id, vec![]);
  let plugin = RocksdbServerDiskPlugin::new(collab_id, db).unwrap();
  collab.lock().add_plugin(Arc::new(plugin));
  collab.async_initialize().await;

  let broadcast = CollabBroadcast::new(object_id, collab.clone(), 10);
  CollabGroup {
    collab,
    broadcast,
    subscribers: Default::default(),
  }
}

pub struct Cleaner {
  path: PathBuf,
  should_clean: bool,
}

impl Cleaner {
  fn new(path: PathBuf) -> Self {
    Self {
      path,
      should_clean: true,
    }
  }

  pub fn set_should_clean(&mut self, should_clean: bool) {
    self.should_clean = should_clean;
  }

  fn cleanup(dir: &PathBuf) {
    let _ = std::fs::remove_dir_all(dir);
  }
}

impl Drop for Cleaner {
  fn drop(&mut self) {
    if self.should_clean {
      Self::cleanup(&self.path)
    }
  }
}
