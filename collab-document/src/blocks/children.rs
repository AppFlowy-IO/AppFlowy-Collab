use collab::preclude::*;
use std::collections::HashMap;

#[derive(Clone)]
pub struct ChildrenOperation {
  root: MapRefWrapper,
}

impl ChildrenOperation {
  pub fn new(root: MapRefWrapper) -> Self {
    Self { root }
  }

  /// get the children of a block with the given id or create it if it does not exist
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

  /// get all the children of the root block
  pub fn get_all_children(&self) -> HashMap<String, Vec<String>> {
    let txn = self.root.transact();
    self
      .root
      .iter(&txn)
      .filter_map(|(k, _)| {
        self.root.get_array_ref_with_txn(&txn, k).map(|children| {
          (
            k.to_string(),
            children
              .iter(&txn)
              .map(|child| child.to_string(&txn))
              .collect(),
          )
        })
      })
      .collect()
  }

  pub fn create_children_with_txn(
    &self,
    txn: &mut TransactionMut,
    children_id: &str,
  ) -> ArrayRefWrapper {
    self
      .root
      .insert_array_with_txn(txn, children_id, Vec::<String>::new())
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
    self
      .root
      .get_array_ref_with_txn(txn, children_id)
      .and_then(|children| {
        children
          .iter(txn)
          .position(|child| child.to_string(txn) == child_id)
      })
      .map(|index| index as u32)
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
    if let Some(index) = self.get_child_index_with_txn(txn, children_id, child_id) {
      children_ref.remove_with_txn(txn, index);
    }
  }
}
