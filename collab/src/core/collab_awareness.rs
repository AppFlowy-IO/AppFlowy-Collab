use std::ops::Deref;
use std::sync::Arc;

use parking_lot::Mutex;
use serde_json::Value;

use crate::core::collab::CollabOrigin;
use crate::preclude::{Collab, CollabPlugin};

#[derive(Clone)]
pub struct MutexCollab(Arc<Mutex<Collab>>);

impl MutexCollab {
  pub fn new(origin: CollabOrigin, object_id: &str, plugins: Vec<Arc<dyn CollabPlugin>>) -> Self {
    let collab = Collab::new(origin.uid, object_id, plugins).with_device_id(origin.device_id);
    MutexCollab(Arc::new(Mutex::new(collab)))
  }

  pub fn initial(&self) {
    self.0.lock().initial();
  }

  pub fn to_json_value(&self) -> Value {
    self.0.lock().to_json_value()
  }
}

impl Deref for MutexCollab {
  type Target = Arc<Mutex<Collab>>;
  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

unsafe impl Sync for MutexCollab {}

unsafe impl Send for MutexCollab {}
