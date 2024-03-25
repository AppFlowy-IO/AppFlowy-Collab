use std::collections::HashMap;
use std::sync::{Arc, Once};

use bytes::Bytes;
use collab::core::collab::DocStateSource;
use collab::core::origin::CollabOrigin;
use collab::preclude::*;
use collab::util::deserialize_i32_from_numeric;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use tracing_subscriber::fmt::Subscriber;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;
use yrs::updates::decoder::Decode;

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct TaskInfo {
  pub title: String,
  pub repeated: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Person {
  pub(crate) name: String,
  pub(crate) position: Position,
}

#[derive(Default, Debug, Serialize, Deserialize)]
pub struct Position {
  pub(crate) title: String,
  #[serde(deserialize_with = "deserialize_i32_from_numeric")]
  pub(crate) level: i32,
}

#[derive(Debug, Default, Clone)]
pub struct CollabStateCachePlugin(Arc<RwLock<Vec<Bytes>>>);

impl CollabStateCachePlugin {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn get_doc_state(&self) -> Result<DocStateSource, anyhow::Error> {
    let mut updates = vec![];
    for encoded_data in self.0.read().iter() {
      updates.push(encoded_data.to_vec());
    }

    let updates = updates
      .iter()
      .map(|update| update.as_ref())
      .collect::<Vec<&[u8]>>();

    let doc_state = merge_updates_v1(&updates)
      .map_err(|err| anyhow::anyhow!("merge updates failed: {:?}", err))
      .unwrap();
    Ok(DocStateSource::FromDocState(doc_state))
  }

  #[allow(dead_code)]
  pub fn get_update(&self) -> Result<Update, anyhow::Error> {
    let read_guard = self.0.read();
    let updates = read_guard
      .iter()
      .map(|update| update.as_ref())
      .collect::<Vec<&[u8]>>();
    let encoded_data = merge_updates_v1(&updates)?;
    let update = Update::decode_v1(&encoded_data)?;
    Ok(update)
  }
}

impl CollabPlugin for CollabStateCachePlugin {
  fn receive_update(&self, _object_id: &str, txn: &TransactionMut, update: &[u8]) {
    let mut write_guard = self.0.write();
    if write_guard.is_empty() {
      let doc_state = txn.encode_state_as_update_v1(&StateVector::default());
      write_guard.push(Bytes::from(doc_state));
    }
    write_guard.push(Bytes::from(update.to_vec()));
  }

  fn receive_local_state(
    &self,
    _origin: &CollabOrigin,
    _object_id: &str,
    _event: &yrs::sync::awareness::Event,
  ) {
  }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Document2 {
  doc_id: String,
  name: String,
  tasks: HashMap<String, TaskInfo>,
}

#[cfg(test)]
mod tests {
  use crate::util::{Document2, TaskInfo};

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
    println!("{:?}", tasks);

    let a = serde_json::from_value::<Document2>(json).unwrap();

    println!("{:?}", a);
  }
}

#[allow(clippy::items_after_test_module)]
pub fn setup_log() {
  static START: Once = Once::new();
  START.call_once(|| {
    let level = "info";
    let mut filters = vec![];
    filters.push(format!("collab={}", level));
    std::env::set_var("RUST_LOG", filters.join(","));

    let subscriber = Subscriber::builder()
      .with_env_filter(EnvFilter::from_default_env())
      .with_ansi(true)
      .finish();
    subscriber.try_init().unwrap();
  });
}
