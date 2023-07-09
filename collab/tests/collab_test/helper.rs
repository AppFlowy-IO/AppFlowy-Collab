use std::collections::HashMap;
use std::sync::{Arc, Once};

use bytes::Bytes;
use collab::core::collab::MutexCollab;
use collab::preclude::*;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use yrs::updates::decoder::Decode;

use tracing_subscriber::fmt::Subscriber;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;

use crate::struct_define::{Document, Owner, TaskInfo};

#[derive(Debug, Serialize, Deserialize)]
pub struct Person {
  pub(crate) name: String,
  pub(crate) position: Position,
}

#[derive(Default, Debug, Serialize, Deserialize)]
pub struct Position {
  pub(crate) title: String,
  pub(crate) level: u8,
}

pub fn make_collab_pair() -> (MutexCollab, MutexCollab, CollabStateCachePlugin) {
  let update_cache = CollabStateCachePlugin::new();
  let local_collab = CollabBuilder::new(1, "1")
    .with_plugin(update_cache.clone())
    .build()
    .unwrap();
  local_collab.lock().initialize();

  // Insert document
  local_collab
    .lock()
    .insert_json_with_path(vec![], "document", test_document());
  let updates = update_cache.get_updates();
  let remote_collab = CollabBuilder::new(1, "1")
    .with_raw_data(updates.unwrap())
    .build()
    .unwrap();
  remote_collab.lock().initialize();

  (local_collab, remote_collab, update_cache)
}

#[derive(Debug, Default, Clone)]
pub struct CollabStateCachePlugin(Arc<RwLock<Vec<Bytes>>>);

impl CollabStateCachePlugin {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn get_updates(&self) -> Result<Vec<Vec<u8>>, anyhow::Error> {
    let mut updates = vec![];
    for encoded_data in self.0.read().iter() {
      updates.push(encoded_data.to_vec());
    }
    Ok(updates)
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

  pub fn clear(&self) {
    self.0.write().clear()
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
}

fn test_document() -> Document {
  let owner = Owner {
    id: "owner_id".to_string(),
    name: "nathan".to_string(),
    email: "nathan@appflowy.io".to_string(),
    location: None,
  };

  let mut attributes = HashMap::new();
  attributes.insert("1".to_string(), "task 1".to_string());
  attributes.insert("2".to_string(), "task 2".to_string());

  let mut tasks = HashMap::new();
  tasks.insert(
    "1".to_string(),
    TaskInfo {
      title: "Task 1".to_string(),
      repeated: true,
    },
  );
  tasks.insert(
    "2".to_string(),
    TaskInfo {
      title: "Task 2".to_string(),
      repeated: false,
    },
  );

  Document {
    doc_id: "doc_id".to_string(),
    name: "Hello world".to_string(),
    created_at: 0,
    attributes,
    tasks,
    owner,
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
  use crate::helper::{Document2, TaskInfo};

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
