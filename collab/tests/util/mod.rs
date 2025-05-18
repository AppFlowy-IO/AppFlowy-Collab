use std::collections::HashMap;
use std::sync::{Arc, Once, RwLock};

use bytes::Bytes;

use collab::core::collab::DataSource;

use collab::core::collab_plugin::CollabPluginType;
use collab::preclude::*;
use serde::{Deserialize, Serialize};
use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt::Subscriber;
use tracing_subscriber::util::SubscriberInitExt;
use yrs::updates::decoder::Decode;

#[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct TaskInfo {
  pub title: String,
  pub repeated: bool,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct Person {
  pub(crate) name: String,
  pub(crate) position: Position,
}

#[derive(Default, Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct Position {
  pub(crate) title: String,
  #[serde(deserialize_with = "collab::preclude::deserialize_i32_from_numeric")]
  pub(crate) level: i32,
}

#[derive(Debug, Default, Clone)]
pub struct CollabStateCachePlugin(Arc<RwLock<Vec<Bytes>>>);

impl CollabStateCachePlugin {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn get_doc_state(&self) -> Result<DataSource, anyhow::Error> {
    let mut updates = vec![];
    let lock = self.0.read().unwrap();
    for encoded_data in lock.iter() {
      updates.push(encoded_data.as_ref());
    }

    let doc_state = merge_updates_v1(&updates)
      .map_err(|err| anyhow::anyhow!("merge updates failed: {:?}", err))
      .unwrap();
    Ok(DataSource::DocStateV1(doc_state))
  }

  #[allow(dead_code)]
  pub fn get_update(&self) -> Result<Update, anyhow::Error> {
    let read_guard = self.0.read().unwrap();
    let updates = read_guard
      .iter()
      .map(|update| update.as_ref())
      .collect::<Vec<&[u8]>>();
    let encoded_data = merge_updates_v1(updates)?;
    let update = Update::decode_v1(&encoded_data)?;
    Ok(update)
  }
}

impl CollabPlugin for CollabStateCachePlugin {
  fn receive_update(&self, _object_id: &str, txn: &TransactionMut, update: &[u8]) {
    let mut write_guard = self.0.write().unwrap();
    if write_guard.is_empty() {
      let doc_state = txn.encode_state_as_update_v1(&StateVector::default());
      write_guard.push(Bytes::from(doc_state));
    }
    write_guard.push(Bytes::from(update.to_vec()));
  }

  fn plugin_type(&self) -> CollabPluginType {
    CollabPluginType::Other("CollabStateCachePlugin".to_string())
  }
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Document2 {
  doc_id: String,
  name: String,
  tasks: HashMap<String, TaskInfo>,
}

#[cfg(test)]
mod tests {
  use crate::util::{Document2, TaskInfo};
  use serde_json::json;
  use std::collections::HashMap;

  #[test]
  fn test() {
    let mut doc = Document2 {
      doc_id: "".to_string(),
      name: "".to_string(),
      tasks: Default::default(),
    };

    doc.tasks.insert(
      "1".to_string(),
      TaskInfo {
        title: "Task 1".to_string(),
        repeated: false,
      },
    );
    let json = serde_json::to_value(&doc).unwrap();
    let tasks = &json["tasks"]["1"];
    assert_eq!(tasks, &json!({"repeated": false, "title": "Task 1"}));

    let a = serde_json::from_value::<Document2>(json).unwrap();
    assert_eq!(
      a,
      Document2 {
        doc_id: "".to_string(),
        name: "".to_string(),
        tasks: HashMap::from([(
          "1".to_string(),
          TaskInfo {
            title: "Task 1".to_string(),
            repeated: false,
          }
        )]),
      }
    );
  }
}

#[allow(clippy::items_after_test_module)]
pub fn setup_log() {
  static START: Once = Once::new();
  START.call_once(|| {
    let level = "info";
    let mut filters = vec![];
    filters.push(format!("collab={}", level));
    unsafe {
      std::env::set_var("RUST_LOG", filters.join(","));
    }

    let subscriber = Subscriber::builder()
      .with_env_filter(EnvFilter::from_default_env())
      .with_ansi(true)
      .finish();
    subscriber.try_init().unwrap();
  });
}
