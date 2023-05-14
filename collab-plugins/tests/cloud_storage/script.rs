use crate::setup_log;
use collab::core::collab::MutexCollab;
use collab::core::origin::{CollabClient, CollabOrigin};
use collab::preclude::Collab;
use collab_plugins::cloud_storage_plugin::{get_aws_remote_doc, AWSDynamoDBPlugin};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

pub enum TestScript {
  CreateCollab {
    uid: i64,
    object_id: String,
  },
  ModifyCollab {
    uid: i64,
    object_id: String,
    f: fn(&Collab),
  },
  Wait {
    secs: u64,
  },
  AssertLocal {
    object_id: String,
    expected: Value,
  },
  AssertRemote {
    object_id: String,
    expected: Value,
  },
}

pub struct CloudStorageTest {
  collab_by_object_id: HashMap<String, Arc<MutexCollab>>,
}

impl CloudStorageTest {
  pub fn new() -> Self {
    setup_log();
    Self {
      collab_by_object_id: HashMap::new(),
    }
  }

  pub async fn run_script(&mut self, script: TestScript) {
    match script {
      TestScript::CreateCollab { uid, object_id } => {
        let origin = CollabOrigin::Client(CollabClient::new(uid, "1"));
        let local_collab = Arc::new(MutexCollab::new(origin, &object_id, vec![]));
        let plugin = AWSDynamoDBPlugin::new(object_id.clone(), local_collab.clone())
          .await
          .unwrap();
        local_collab.lock().add_plugin(Arc::new(plugin));
        local_collab.initial();
        self.collab_by_object_id.insert(object_id, local_collab);
      },
      TestScript::ModifyCollab {
        uid: _,
        object_id,
        f,
      } => {
        let collab = self.collab_by_object_id.get(&object_id).unwrap().lock();
        f(&collab);
      },
      TestScript::Wait { secs } => {
        tokio::time::sleep(Duration::from_secs(secs)).await;
      },
      TestScript::AssertLocal {
        object_id,
        expected,
      } => {
        let collab = self.collab_by_object_id.get(&object_id).unwrap().lock();
        assert_json_diff::assert_json_eq!(collab.to_json_value(), expected,);
      },
      TestScript::AssertRemote {
        object_id,
        expected,
      } => {
        let collab = get_aws_remote_doc(&object_id).await;
        let json = collab.lock().to_json_value();
        assert_json_diff::assert_json_eq!(json, expected,);
      },
    }
  }

  pub async fn run_scripts(&mut self, scripts: Vec<TestScript>) {
    for script in scripts {
      self.run_script(script).await;
    }
  }
}
