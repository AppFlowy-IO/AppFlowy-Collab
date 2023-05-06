use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use collab::core::origin::CollabClient;
use collab::preclude::Collab;
use collab_persistence::kv::rocks_kv::RocksCollabDB;
use serde_json::Value;

use crate::util::{spawn_server, TestClient, TestServer};

pub enum TestScript {
  CreateClient {
    uid: i64,
    device_id: String,
  },
  CreateEmptyClient {
    uid: i64,
    device_id: String,
  },
  CreateClientWithDb {
    uid: i64,
    device_id: String,
    db: Arc<RocksCollabDB>,
  },
  DisconnectClient {
    device_id: String,
  },
  ConnectClient {
    device_id: String,
  },
  Wait {
    secs: u64,
  },
  AssertClientContent {
    device_id: String,
    expected: Value,
  },
  AssertServerContent {
    expected: Value,
  },
  ModifyLocalCollab {
    device_id: String,
    f: fn(&Collab),
  },
  ModifyRemoteCollab {
    f: fn(&Collab),
  },
  AssertClientEqualToServer {
    device_id: String,
  },
  AssertClientEqual {
    device_id_a: String,
    device_id_b: String,
  },
}

pub struct ScriptTest {
  object_id: String,
  server: TestServer,
  pub clients: HashMap<String, TestClient>,
}

impl ScriptTest {
  pub async fn new(_collab_id: i64, object_id: &str) -> Self {
    let server = spawn_server(object_id).await.unwrap();
    Self {
      object_id: object_id.to_string(),
      server,
      clients: HashMap::new(),
    }
  }

  pub fn remove_client(&mut self, device_id: &str) -> TestClient {
    self.clients.remove(device_id).unwrap()
  }

  pub async fn run_script(&mut self, script: TestScript) {
    match script {
      TestScript::CreateClient { uid, device_id } => {
        let origin = CollabClient::new(uid, &device_id);
        let client = TestClient::new(origin, &self.object_id, self.server.address, true)
          .await
          .unwrap();
        self.clients.insert(device_id.to_string(), client);
      },
      TestScript::CreateEmptyClient { uid, device_id } => {
        let origin = CollabClient::new(uid, &device_id);
        let client = TestClient::new(origin, &self.object_id, self.server.address, false)
          .await
          .unwrap();
        self.clients.insert(device_id.to_string(), client);
      },
      TestScript::CreateClientWithDb { uid, device_id, db } => {
        let origin = CollabClient::new(uid, &device_id);
        let new_client = TestClient::with_db(origin, &self.object_id, self.server.address, db)
          .await
          .unwrap();
        let _ = self.clients.insert(device_id.to_string(), new_client);
      },
      TestScript::DisconnectClient { device_id } => {
        if let Some(client) = self.clients.get_mut(&device_id) {
          client.disconnect()
        }
      },
      TestScript::ConnectClient { device_id } => {
        if let Some(client) = self.clients.get_mut(&device_id) {
          client.connect()
        }
      },
      TestScript::AssertClientContent {
        device_id,
        expected,
      } => {
        let client = self.clients.get_mut(&device_id).unwrap();
        let json = client.to_json_value();
        assert_json_diff::assert_json_eq!(json, expected,);
      },
      TestScript::AssertServerContent { expected } => {
        let server_json = self.server.get_doc_json(&self.object_id);
        assert_json_diff::assert_json_eq!(server_json, expected,);
      },
      TestScript::AssertClientEqualToServer { device_id } => {
        let client = self.clients.get_mut(&device_id).unwrap();
        let client_json = client.to_json_value();

        let server_json = self.server.get_doc_json(&self.object_id);
        assert_eq!(client_json, server_json);
      },
      TestScript::Wait { secs } => {
        tokio::time::sleep(Duration::from_secs(secs)).await;
      },
      TestScript::ModifyLocalCollab { device_id, f } => {
        let client = self.clients.get_mut(&device_id).unwrap();
        f(&client.lock());
      },
      TestScript::ModifyRemoteCollab { f } => {
        self
          .server
          .groups
          .get_mut(&self.object_id)
          .unwrap()
          .get_mut_collab(f);
      },
      TestScript::AssertClientEqual {
        device_id_a,
        device_id_b,
      } => {
        let client_a = self.clients.get_mut(&device_id_a).unwrap().to_json_value();
        let client_b = self.clients.get_mut(&device_id_b).unwrap().to_json_value();
        assert_eq!(client_a, client_b);
      },
    }
  }

  pub async fn run_scripts(&mut self, scripts: Vec<TestScript>) {
    for script in scripts {
      self.run_script(script).await;
    }
  }
}
