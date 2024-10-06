use std::collections::HashMap;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;

use collab::preclude::{Any, MapExt, MapRef, YrsValue};
use collab::preclude::{Array, ArrayRef, ReadTxn, TransactionMut};
use serde::{Deserialize, Serialize};

/// Used to keep track of the view hierarchy.
/// Parent-child relationship is stored in the map and each child is stored in an array.
/// relation {
///   parent_id: [child_id1, child_id2, ...]
///   parent_id: [child_id1, child_id2, ...]
/// }
///
pub struct ParentChildRelations {
  container: MapRef,
}

impl ParentChildRelations {
  pub fn new(container: MapRef) -> Self {
    Self { container }
  }

  /// Dissociates a parent-child relationship within a given transaction.
  ///
  /// The `ViewIdentifier` object representing the child view is returned, provided the child view
  /// is successfully dissociated. If the child view is not present within the parent,
  /// a warning is issued and the function still returns the `ViewIdentifier` object.
  ///
  /// # Arguments
  ///
  /// * `txn` - A mutable reference to a transaction.
  /// * `parent_id` - A string slice that holds the id of the parent view.
  /// * `view_id` - A string slice that holds the id of the child view to be dissociated from the parent.
  ///
  /// # Returns
  ///
  /// This function returns an `Option<ViewIdentifier>` which represents the child view that was dissociated
  /// from the parent. If the child view was not originally part of the parent,
  /// the `Option<ViewIdentifier>` will still be returned, containing the child view's id.
  ///
  pub fn dissociate_parent_child_with_txn(
    &self,
    txn: &mut TransactionMut,
    parent_id: &str,
    view_id: &str,
  ) -> Option<ViewIdentifier> {
    let child = ViewIdentifier {
      id: view_id.to_string(),
    };
    if let Some(children) = self.get_children_with_txn(txn, parent_id) {
      let index = children
        .get_children_with_txn(txn)
        .items
        .iter()
        .position(|i| i.id == view_id);
      match index {
        None => {
          tracing::warn!("🟡 The view {} is not in parent {}.", view_id, parent_id);
        },
        Some(i) => {
          children.remove_child_with_txn(txn, i as u32);
        },
      };
    }
    Some(child)
  }

  /// Associates a parent-child relationship within a given transaction.
  ///
  /// an optional `prev_view_id` as inputs, and attempts to associate a parent view with a child view
  /// in the context of a transaction. The child view is placed in the list of children after the view identified by `prev_view_id`.
  ///
  /// If `prev_view_id` is not provided (`None`), the child view is placed at the start of the list of children.
  ///
  /// # Arguments
  ///
  /// * `txn` - A mutable reference to a transaction.
  /// * `parent_id` - A string slice that holds the id of the parent view.
  /// * `view_id` - A string slice that holds the id of the child view to be associated with the parent.
  /// * `prev_view_id` - An `Option<String>` that holds the id of the view after which the child view will be placed.
  ///
  pub fn associate_parent_child_with_txn(
    &self,
    txn: &mut TransactionMut,
    parent_id: &str,
    view_id: &str,
    prev_view_id: Option<String>,
  ) {
    if let Some(children) = self.get_children_with_txn(txn, parent_id) {
      let prev_index = match prev_view_id {
        None => None,
        Some(prev_id) => children
          .get_children_with_txn(txn)
          .items
          .iter()
          .position(|i| i.id == prev_id),
      };
      let index = match prev_index {
        None => 0,
        Some(index) => (index + 1) as u32,
      };
      let child = ViewIdentifier {
        id: view_id.to_string(),
      };
      children.insert_child_with_txn(txn, index, child);
    }
  }

  pub fn move_child_with_txn(&self, txn: &mut TransactionMut, parent_id: &str, from: u32, to: u32) {
    if let Some(belonging_array) = self.get_children_with_txn(txn, parent_id) {
      belonging_array.move_child_with_txn(txn, from, to);
    }
  }

  pub fn get_children_with_txn<T: ReadTxn>(
    &self,
    txn: &T,
    parent_id: &str,
  ) -> Option<ChildrenArray> {
    let array = self.container.get_with_txn(txn, parent_id)?;
    Some(ChildrenArray::from_array(array))
  }

  pub fn get_or_create_children_with_txn(
    &self,
    txn: &mut TransactionMut,
    parent_id: &str,
  ) -> ChildrenArray {
    let array_ref: ArrayRef = self
      .container
      .get_with_txn(txn, parent_id)
      .unwrap_or_else(|| self.container.get_or_init_array(txn, parent_id));
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
    index: Option<u32>,
  ) {
    let array = self.get_or_create_children_with_txn(txn, parent_id);
    array.add_children_with_txn(txn, children, index);
  }
}

/// Handy wrapper around an array of children.
/// It provides methods to manipulate the array.
#[derive(Clone)]
pub struct ChildrenArray(ArrayRef);

impl ChildrenArray {
  pub fn from_array(array: ArrayRef) -> Self {
    Self(array)
  }

  pub fn get_children_with_txn<T: ReadTxn>(&self, txn: &T) -> RepeatedViewIdentifier {
    children_from_array_ref(txn, &self.0)
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
    let value = self.0.get(txn, index)?;
    self.0.remove(txn, index);
    view_identifier_from_value(value)
  }

  pub fn insert_child_with_txn(&self, txn: &mut TransactionMut, index: u32, child: ViewIdentifier) {
    self.0.insert(txn, index, child);
  }

  /// Add children to the views.
  ///
  /// if the index is provided, the children will be inserted at the index.
  /// if the index is None or the index is greater than the length of the array, the children will be appended to the last of the views.
  pub fn add_children_with_txn(
    &self,
    txn: &mut TransactionMut,
    children: Vec<ViewIdentifier>,
    index: Option<u32>,
  ) {
    let mut existing_children_ids: Vec<String> = self
      .get_children_with_txn(txn)
      .into_inner()
      .into_iter()
      .map(|child_view| child_view.id)
      .collect();

    let values = children.into_iter().filter(|child| {
      let contains_child = existing_children_ids.contains(&child.id);
      if !contains_child {
        existing_children_ids.push(child.id.clone());
      }
      !contains_child
    });

    if let Some(index) = index {
      if index < self.0.len(txn) {
        self.0.insert_range(txn, index, values);
        return;
      }
    }

    for value in values {
      self.0.push_back(txn, value);
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
  if let YrsValue::Any(Any::Map(map)) = value {
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

impl From<RepeatedViewIdentifier> for Vec<Any> {
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

impl Deref for ViewIdentifier {
  type Target = String;

  fn deref(&self) -> &Self::Target {
    &self.id
  }
}

impl ViewIdentifier {
  pub fn new(id: String) -> Self {
    Self { id }
  }
  pub fn from_map(map: &HashMap<String, Any>) -> Option<Self> {
    if let Any::String(id) = map.get("id")? {
      return Some(Self { id: id.to_string() });
    }

    None
  }
}

impl From<ViewIdentifier> for Any {
  fn from(value: ViewIdentifier) -> Self {
    let mut map = HashMap::new();
    map.insert("id".to_string(), Any::String(Arc::from(value.id)));
    Any::Map(Arc::new(map))
  }
}
