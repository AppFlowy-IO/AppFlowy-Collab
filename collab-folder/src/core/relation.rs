use std::collections::HashMap;
use std::ops::{Deref, DerefMut};

use collab::preclude::{lib0Any, YrsValue};
use collab::preclude::{Array, ArrayRef, ArrayRefWrapper, MapRefWrapper, ReadTxn, TransactionMut};
use serde::{Deserialize, Serialize};

/// Used to keep track of the view hierarchy.
/// Parent-child relationship is stored in the map and each child is stored in an array.
/// relation {
///   parent_id: [child_id1, child_id2, ...]
///   parent_id: [child_id1, child_id2, ...]
/// }
///
pub struct ViewRelations {
  container: MapRefWrapper,
}

impl ViewRelations {
  pub fn new(container: MapRefWrapper) -> Self {
    Self { container }
  }

  /// Move the child at `from` to `to` within the parent with `parent_id`.
  pub fn move_child(&self, parent_id: &str, from: u32, to: u32) {
    self.container.with_transact_mut(|txn| {
      self.move_child_with_txn(txn, parent_id, from, to);
    })
  }

  pub fn move_child_with_txn(&self, txn: &mut TransactionMut, parent_id: &str, from: u32, to: u32) {
    if let Some(belonging_array) = self.get_children_with_txn(txn, parent_id) {
      belonging_array.move_child_with_txn(txn, from, to);
    }
  }

  /// Returns the children of the parent with `parent_id`.
  /// The children are stored in an array.
  pub fn get_children(&self, parent_id: &str) -> Option<ChildrenArray> {
    let txn = self.container.transact();
    self.get_children_with_txn(&txn, parent_id)
  }

  pub fn get_children_with_txn<T: ReadTxn>(
    &self,
    txn: &T,
    parent_id: &str,
  ) -> Option<ChildrenArray> {
    let array = self.container.get_array_ref_with_txn(txn, parent_id)?;
    Some(ChildrenArray::from_array(array))
  }

  pub fn get_or_create_children_with_txn(
    &self,
    txn: &mut TransactionMut,
    parent_id: &str,
  ) -> ChildrenArray {
    let array_ref = self
      .container
      .get_array_ref_with_txn(txn, parent_id)
      .unwrap_or_else(|| {
        self
          .container
          .insert_array_with_txn::<ViewIdentifier>(txn, parent_id, vec![])
      });
    ChildrenArray::from_array(array_ref)
  }

  pub fn delete_children_with_txn(&self, txn: &mut TransactionMut, parent_id: &str, index: u32) {
    if let Some(belonging_array) = self.get_children_with_txn(txn, parent_id) {
      belonging_array.remove_child_with_txn(txn, index);
    }
  }

  /// Add children to the parent with `parent_id`.
  pub fn add_children(
    &self,
    txn: &mut TransactionMut,
    parent_id: &str,
    children: Vec<ViewIdentifier>,
  ) {
    let array = self.get_or_create_children_with_txn(txn, parent_id);
    array.add_children_with_txn(txn, children);
  }
}

/// Handy wrapper around an array of children.
/// It provides methods to manipulate the array.
#[derive(Clone)]
pub struct ChildrenArray(ArrayRefWrapper);

impl ChildrenArray {
  pub fn from_array(array: ArrayRefWrapper) -> Self {
    Self(array)
  }

  pub fn get_children(&self) -> RepeatedViewIdentifier {
    let txn = self.0.transact();
    self.get_children_with_txn(&txn)
  }

  pub fn get_children_with_txn<T: ReadTxn>(&self, txn: &T) -> RepeatedViewIdentifier {
    children_from_array_ref(txn, &self.0)
  }

  pub fn move_child(&self, from: u32, to: u32) {
    self.0.with_transact_mut(|txn| {
      self.move_child_with_txn(txn, from, to);
    });
  }
  pub fn move_child_with_txn(&self, txn: &mut TransactionMut, from: u32, to: u32) {
    if let Some(YrsValue::Any(value)) = self.0.get(txn, from) {
      self.0.remove(txn, from);
      self.0.insert(txn, to, value);
    }
  }

  pub fn remove_child_with_txn(
    &self,
    txn: &mut TransactionMut,
    index: u32,
  ) -> Option<ViewIdentifier> {
    self
      .0
      .remove_with_txn(txn, index)
      .and_then(view_identifier_from_value)
  }

  pub fn remove_child(&self, index: u32) {
    self.0.with_transact_mut(|txn| {
      self.0.remove_with_txn(txn, index);
    })
  }

  pub fn add_children(&self, belongings: Vec<ViewIdentifier>) {
    self
      .0
      .with_transact_mut(|txn| self.add_children_with_txn(txn, belongings))
  }

  pub fn add_children_with_txn(&self, txn: &mut TransactionMut, children: Vec<ViewIdentifier>) {
    let mut existing_children_ids = self
      .get_children_with_txn(txn)
      .into_inner()
      .into_iter()
      .map(|child_view| child_view.id)
      .collect::<Vec<String>>();

    for child in children {
      if !existing_children_ids.contains(&child.id) {
        existing_children_ids.push(child.id.clone());
        self.0.push_back(txn, child);
      }
    }
  }
}

pub fn children_from_array_ref<T: ReadTxn>(
  txn: &T,
  array_ref: &ArrayRef,
) -> RepeatedViewIdentifier {
  let mut children = RepeatedViewIdentifier::new(vec![]);
  for value in array_ref.iter(txn) {
    if let Some(identifier) = view_identifier_from_value(value) {
      children.items.push(identifier);
    }
  }
  children
}

pub fn view_identifier_from_value(value: YrsValue) -> Option<ViewIdentifier> {
  if let YrsValue::Any(lib0Any::Map(map)) = value {
    ViewIdentifier::from_map(&map)
  } else {
    None
  }
}

#[derive(Serialize, Deserialize, Default, Clone, Eq, PartialEq, Debug)]
#[repr(transparent)]
pub struct RepeatedViewIdentifier {
  pub items: Vec<ViewIdentifier>,
}

impl RepeatedViewIdentifier {
  pub fn new(items: Vec<ViewIdentifier>) -> Self {
    Self { items }
  }

  pub fn into_inner(self) -> Vec<ViewIdentifier> {
    self.items
  }
}

impl Deref for RepeatedViewIdentifier {
  type Target = Vec<ViewIdentifier>;

  fn deref(&self) -> &Self::Target {
    &self.items
  }
}

impl DerefMut for RepeatedViewIdentifier {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.items
  }
}

impl From<RepeatedViewIdentifier> for Vec<lib0Any> {
  fn from(values: RepeatedViewIdentifier) -> Self {
    values
      .into_inner()
      .into_iter()
      .map(|value| value.into())
      .collect::<Vec<_>>()
  }
}

#[derive(Serialize, Deserialize, Default, Clone, Eq, PartialEq, Debug)]
pub struct ViewIdentifier {
  pub id: String,
}

impl ViewIdentifier {
  pub fn new(id: String) -> Self {
    Self { id }
  }
  pub fn from_map(map: &HashMap<String, lib0Any>) -> Option<Self> {
    if let lib0Any::String(id) = map.get("id")? {
      return Some(Self { id: id.to_string() });
    }

    None
  }
}

impl From<ViewIdentifier> for lib0Any {
  fn from(value: ViewIdentifier) -> Self {
    let mut map = HashMap::new();
    map.insert("id".to_string(), lib0Any::String(value.id.into_boxed_str()));
    lib0Any::Map(Box::new(map))
  }
}
