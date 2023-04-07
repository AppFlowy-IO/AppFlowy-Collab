use collab::preclude::*;
use std::collections::HashMap;

pub struct ChildrenOperation {
  pub root: MapRefWrapper,
}

impl ChildrenOperation {
  pub fn new(root: MapRefWrapper) -> Self {
    Self { root }
  }

  pub fn get_children_with_txn(
    &self,
    txn: &mut TransactionMut,
    children_id: &str,
  ) -> ArrayRefWrapper {
    self
      .root
      .get_array_ref_with_txn(txn, children_id)
      .unwrap_or_else(|| self.create_children_with_txn(txn, children_id))
  }

  pub fn get_all_children(&self) -> HashMap<String, Vec<String>> {
    let txn = self.root.transact();
    let mut hash_map = HashMap::new();
    self.root.iter(&txn).for_each(|(k, _)| {
      let children = self.root.get_array_ref_with_txn(&txn, k);
      if let Some(children) = children {
        hash_map.insert(
          k.to_string(),
          children
            .iter(&txn)
            .map(|child| child.to_string(&txn))
            .collect(),
        );
      }
    });
    hash_map
  }

  pub fn create_children_with_txn(
    &self,
    txn: &mut TransactionMut,
    children_id: &str,
  ) -> ArrayRefWrapper {
    let children: Vec<String> = vec![];
    self.root.insert_array_with_txn(txn, children_id, children)
  }

  pub fn delete_children_with_txn(&self, txn: &mut TransactionMut, children_id: &str) {
    self.root.delete_with_txn(txn, children_id);
  }

  pub fn get_child_index_with_txn<T: ReadTxn>(
    &self,
    txn: &T,
    children_id: &str,
    child_id: &str,
  ) -> Option<u32> {
    let children_ref = self.root.get_array_ref_with_txn(txn, children_id);
    if children_ref.as_ref()?.len(txn) == 0 {
      return None;
    }
    let children_ref = children_ref.unwrap();

    let index = children_ref
      .iter(txn)
      .position(|child| child.to_string(txn) == child_id);

    index.map(|index| index as u32)
  }

  pub fn insert_child_with_txn(
    &self,
    txn: &mut TransactionMut,
    children_id: &str,
    child_id: &str,
    index: u32,
  ) {
    let children_ref = self.get_children_with_txn(txn, children_id);
    children_ref.insert(txn, index, child_id);
  }

  pub fn delete_child_with_txn(&self, txn: &mut TransactionMut, children_id: &str, child_id: &str) {
    let children_ref = self.get_children_with_txn(txn, children_id);
    let index = self.get_child_index_with_txn(txn, children_id, child_id);
    if let Some(index) = index {
      children_ref.remove_with_txn(txn, index);
    }
  }
}
