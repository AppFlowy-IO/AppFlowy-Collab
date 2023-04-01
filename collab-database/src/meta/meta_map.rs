use collab::preclude::{Doc, MapRef, MapRefWrapper};
use std::ops::Deref;

pub struct MetaMap {
  container: MapRefWrapper,
}

impl MetaMap {
  pub fn new(container: MapRefWrapper) -> Self {
    Self { container }
  }

  pub fn insert_doc(&self, doc: Doc) {
    self.container.insert("1", doc);
  }
}

impl Deref for MetaMap {
  type Target = MapRef;

  fn deref(&self) -> &Self::Target {
    &self.container
  }
}
