use collab::preclude::{ArrayRefWrapper, MapRefWrapper};

pub struct ViewMap {
  container: MapRefWrapper,
}

impl ViewMap {
  pub fn new(container: MapRefWrapper) -> Self {
    // let field_order = FieldOrderArray::new(field_order);
    Self { container }
  }
}
