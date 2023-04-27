use std::ops::{Deref, DerefMut};
use std::sync::Arc;

use parking_lot::Mutex;
use serde_json::Value;

use y_sync::awareness::Awareness;

use crate::preclude::{Collab, CollabPlugin};

pub struct CollabAwareness {
  pub collab: Collab,
  pub awareness: Awareness,
}

impl CollabAwareness {
  pub fn new(uid: i64, object_id: &str, plugins: Vec<Arc<dyn CollabPlugin>>) -> Self {
    let collab = Collab::new(uid, object_id, plugins);
    let awareness = Awareness::new(collab.get_doc().clone());
    CollabAwareness { collab, awareness }
  }
}

impl Deref for CollabAwareness {
  type Target = Awareness;
  fn deref(&self) -> &Self::Target {
    &self.awareness
  }
}

impl DerefMut for CollabAwareness {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.awareness
  }
}

#[derive(Clone)]
pub struct MutexCollabAwareness(Arc<Mutex<CollabAwareness>>);

impl MutexCollabAwareness {
  pub fn new(uid: i64, object_id: &str, plugins: Vec<Arc<dyn CollabPlugin>>) -> Self {
    let awareness = CollabAwareness::new(uid, object_id, plugins);
    MutexCollabAwareness(Arc::new(Mutex::new(awareness)))
  }

  pub fn initial(&self) {
    self.0.lock().collab.initial();
  }

  pub fn to_json_value(&self) -> Value {
    self.0.lock().collab.to_json_value()
  }
}

impl Deref for MutexCollabAwareness {
  type Target = Arc<Mutex<CollabAwareness>>;
  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

unsafe impl Sync for MutexCollabAwareness {}

unsafe impl Send for MutexCollabAwareness {}
