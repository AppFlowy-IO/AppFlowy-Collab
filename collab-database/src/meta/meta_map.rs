use collab::preclude::{Doc, MapRefWrapper};

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
