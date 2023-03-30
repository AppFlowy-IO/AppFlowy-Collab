use collab::preclude::*;
pub struct ChildrenMap {
  pub root: MapRefWrapper,
}
impl ChildrenMap {
  pub fn new(root: MapRefWrapper) -> Self {
    Self { root }
  }

  pub fn to_json(&self) -> serde_json::Value {
    let mut obj = serde_json::json!({});
    let txn = self.root.transact();
    self.root.iter(&txn).for_each(|(key, _)| {
      let key = key.to_string();
      let children = self.root.get_array_ref_with_txn(&txn, &key);
      match children {
        Some(children) => {
          let children = serde_json::json!(children
            .iter(&txn)
            .map(|child| child.to_string(&txn))
            .collect::<Vec<String>>());
          obj[key] = children;
        },
        None => {},
      }
    });
    obj
  }

  pub fn get_children_with_txn(
    &self,
    txn: &mut TransactionMut,
    children_id: &str,
  ) -> ArrayRefWrapper {
    self
      .root
      .get_array_ref_with_txn(txn, children_id)
      .unwrap_or_else(|| self.create_children_with_txn(txn, children_id.to_owned()))
  }

  pub fn create_children_with_txn(
    &self,
    txn: &mut TransactionMut,
    children_id: String,
  ) -> ArrayRefWrapper {
    let children: Vec<String> = vec![];
    self.root.insert_array_with_txn(txn, &children_id, children)
  }

  pub fn delete_children_with_txn(&self, txn: &mut TransactionMut, children_id: &str) {
    self.root.delete_with_txn(txn, children_id);
  }

  pub fn get_child_index(&self, children_id: &str, child_id: &str) -> Option<u32> {
    let children_ref = self.root.get_array_ref(children_id).unwrap();
    let txn = self.root.transact();
    let index = children_ref
      .iter(&txn)
      .position(|child| child.to_string(&txn) == child_id);

    match index {
      Some(index) => Some(index as u32),
      None => None,
    }
  }

  pub fn insert_child_with_txn(
    &self,
    txn: &mut TransactionMut,
    children_id: &str,
    child_id: &str,
    index: u32,
  ) {
    let children_ref = self.get_children_with_txn(txn, children_id);
    children_ref.insert_with_txn(txn, index, child_id);
  }

  pub fn delete_child_with_txn(&self, txn: &mut TransactionMut, children_id: &str, child_id: &str) {
    let children_ref = self.get_children_with_txn(txn, children_id);
    let index = self.get_child_index(children_id, child_id);
    match index {
      Some(index) => children_ref.remove_with_txn(txn, index),
      None => (),
    }
  }
}
