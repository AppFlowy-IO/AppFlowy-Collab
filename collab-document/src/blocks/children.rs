use collab::preclude::*;
use std::collections::HashMap;

#[derive(Clone)]
pub struct ChildrenOperation {
  root: MapRef,
}

impl ChildrenOperation {
  pub fn new(root: MapRef) -> Self {
    Self { root }
  }

  pub fn get_children<T: ReadTxn>(&self, txn: &T, children_id: &str) -> Vec<YrsValue> {
    match self.root.get(txn, children_id) {
      Some(Out::YArray(children)) => children.iter(txn).collect(),
      _ => Vec::default(),
    }
  }

  /// get the children of a block with the given id or create it if it does not exist
  pub fn get_or_init_children(&self, txn: &mut TransactionMut, children_id: &str) -> ArrayRef {
    self.root.get_or_init_array(txn, children_id)
  }

  /// get children map of current root map
  pub fn get_all_children<T: ReadTxn>(&self, txn: &T) -> HashMap<String, Vec<String>> {
    self
      .root
      .iter(txn)
      .filter_map(|(k, _)| {
        self
          .root
          .get_with_txn::<T, ArrayRef>(txn, k)
          .map(|children| {
            (
              k.to_string(),
              children
                .iter(txn)
                .map(|child| child.to_string(txn))
                .collect(),
            )
          })
      })
      .collect()
  }

  /// Create children map of each block.
  pub fn create_children_with_txn(&self, txn: &mut TransactionMut, children_id: &str) -> ArrayRef {
    self.root.insert(txn, children_id, ArrayPrelim::default())
  }

  /// Delete children map when delete this block.
  pub fn delete_children_with_txn(&self, txn: &mut TransactionMut, children_id: &str) {
    self.root.remove(txn, children_id);
  }

  /// Get child index of current block's children map with given child id.
  pub fn get_child_index_with_txn<T: ReadTxn>(
    &self,
    txn: &T,
    children_id: &str,
    child_id: &str,
  ) -> Option<u32> {
    self
      .root
      .get(txn, children_id)
      .and_then(|children| children.cast().ok())
      .and_then(|children: ArrayRef| {
        children
          .iter(txn)
          .position(|child| child.to_string(txn) == child_id)
      })
      .map(|index| index as u32)
  }

  /// Insert child into current block's children map with given child id and index.
  pub fn insert_child_with_txn(
    &self,
    txn: &mut TransactionMut,
    children_id: &str,
    child_id: &str,
    index: u32,
  ) {
    let children_ref = self.get_or_init_children(txn, children_id);
    children_ref.insert(txn, index, child_id);
  }

  /// Delete child from current block's children map with given child id.
  pub fn delete_child_with_txn(&self, txn: &mut TransactionMut, children_id: &str, child_id: &str) {
    let children_ref = self.get_or_init_children(txn, children_id);
    if let Some(index) = self.get_child_index_with_txn(txn, children_id, child_id) {
      children_ref.remove(txn, index);
    }
  }
}
