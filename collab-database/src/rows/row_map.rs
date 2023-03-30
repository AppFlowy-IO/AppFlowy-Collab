use collab::preclude::MapRefWrapper;

pub struct RowMap {
  container: MapRefWrapper,
}

impl RowMap {
  pub fn new(container: MapRefWrapper) -> Self {
    Self { container }
  }
}
