use crate::preclude::CollabPlugin;
use parking_lot::RwLock;
use std::sync::Arc;
use y_sync::awareness::Awareness;
use yrs::{Doc, Transaction};

#[derive(Clone)]
pub struct AwarenessPlugin {
  awareness: Arc<RwLock<Option<Awareness>>>,
}

impl CollabPlugin for AwarenessPlugin {
  fn did_init(&self, _doc: &Doc, _object_id: &str, _txn: &Transaction) {
    // let mut awareness = Awareness::new(doc.clone());
    // let subscription = awareness.on_update(|a, event| {});
    // *self.awareness.write() = Some();
  }
}
