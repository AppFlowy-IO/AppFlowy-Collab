use collab::preclude::{lib0Any, YrsValue};
use collab::preclude::{Array, ArrayRef, ArrayRefWrapper, MapRefWrapper, ReadTxn, TransactionMut};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::ops::{Deref, DerefMut};

pub struct ChildrenMap {
  container: MapRefWrapper,
}

impl ChildrenMap {
  pub fn new(container: MapRefWrapper) -> Self {
    Self { container }
  }

  pub fn move_child_view(&self, parent_id: &str, from: u32, to: u32) {
    self.container.with_transact_mut(|txn| {
      self.move_child_view_with_txn(txn, parent_id, from, to);
    })
  }

  pub fn move_child_view_with_txn(
    &self,
    txn: &mut TransactionMut,
    parent_id: &str,
    from: u32,
    to: u32,
  ) {
    if let Some(belonging_array) = self.get_children_with_txn(txn, parent_id) {
      belonging_array.move_child_with_txn(txn, from, to);
    }
  }

  pub fn get_children(&self, parent_id: &str) -> Option<ChildViewArray> {
    let txn = self.container.transact();
    self.get_children_with_txn(&txn, parent_id)
  }

  pub fn get_children_with_txn<T: ReadTxn>(
    &self,
    txn: &T,
    parent_id: &str,
  ) -> Option<ChildViewArray> {
    let array = self.container.get_array_ref_with_txn(txn, parent_id)?;
    Some(ChildViewArray::from_array(array))
  }

  pub fn get_or_create_children_with_txn(
    &self,
    txn: &mut TransactionMut,
    parent_id: &str,
  ) -> ChildViewArray {
    let array_ref = self
      .container
      .get_array_ref_with_txn(txn, parent_id)
      .unwrap_or_else(|| {
        self
          .container
          .insert_array_with_txn::<ChildView>(txn, parent_id, vec![])
      });
    ChildViewArray::from_array(array_ref)
  }

  pub fn delete_children_with_txn(&self, txn: &mut TransactionMut, parent_id: &str, index: u32) {
    if let Some(belonging_array) = self.get_children_with_txn(txn, parent_id) {
      belonging_array.remove_child_with_txn(txn, index);
    }
  }

  pub fn add_children(&self, txn: &mut TransactionMut, parent_id: &str, children: Vec<ChildView>) {
    let array = self.get_or_create_children_with_txn(txn, parent_id);
    array.add_children_with_txn(txn, children);
  }
}

#[derive(Clone)]
pub struct ChildViewArray {
  container: ArrayRefWrapper,
}

impl ChildViewArray {
  pub fn from_array(belongings: ArrayRefWrapper) -> Self {
    Self {
      container: belongings,
    }
  }

  pub fn get_children(&self) -> ChildViews {
    let txn = self.container.transact();
    self.get_children_with_txn(&txn)
  }

  pub fn get_children_with_txn<T: ReadTxn>(&self, txn: &T) -> ChildViews {
    children_from_array_ref(txn, &self.container)
  }

  pub fn move_child(&self, from: u32, to: u32) {
    self.container.with_transact_mut(|txn| {
      self.move_child_with_txn(txn, from, to);
    });
  }
  pub fn move_child_with_txn(&self, txn: &mut TransactionMut, from: u32, to: u32) {
    if let Some(YrsValue::Any(value)) = self.container.get(txn, from) {
      self.container.remove(txn, from);
      self.container.insert(txn, to, value);
    }
  }

  pub fn remove_child_with_txn(&self, txn: &mut TransactionMut, index: u32) {
    self.container.remove_with_txn(txn, index);
  }

  pub fn remove_child(&self, index: u32) {
    self.container.with_transact_mut(|txn| {
      self.container.remove_with_txn(txn, index);
    })
  }

  pub fn add_children(&self, belongings: Vec<ChildView>) {
    self
      .container
      .with_transact_mut(|txn| self.add_children_with_txn(txn, belongings))
  }

  pub fn add_children_with_txn(&self, txn: &mut TransactionMut, children: Vec<ChildView>) {
    let mut existing_children_ids = self
      .get_children_with_txn(txn)
      .into_inner()
      .into_iter()
      .map(|belonging| belonging.id)
      .collect::<Vec<String>>();

    for child in children {
      if !existing_children_ids.contains(&child.id) {
        existing_children_ids.push(child.id.clone());
        self.container.push_back(txn, child);
      }
    }
  }
}

pub fn children_from_array_ref<T: ReadTxn>(txn: &T, array_ref: &ArrayRef) -> ChildViews {
  let mut children = ChildViews::new(vec![]);
  for value in array_ref.iter(txn) {
    if let YrsValue::Any(lib0Any::Map(map)) = value {
      if let Some(belonging) = ChildView::from_map(&map) {
        children.items.push(belonging);
      }
    }
  }
  children
}

#[derive(Serialize, Deserialize, Default, Clone, Eq, PartialEq, Debug)]
#[repr(transparent)]
pub struct ChildViews {
  pub items: Vec<ChildView>,
}

impl ChildViews {
  pub fn new(items: Vec<ChildView>) -> Self {
    Self { items }
  }

  pub fn into_inner(self) -> Vec<ChildView> {
    self.items
  }
}

impl Deref for ChildViews {
  type Target = Vec<ChildView>;

  fn deref(&self) -> &Self::Target {
    &self.items
  }
}

impl DerefMut for ChildViews {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.items
  }
}

impl From<ChildViews> for Vec<lib0Any> {
  fn from(values: ChildViews) -> Self {
    values
      .into_inner()
      .into_iter()
      .map(|value| value.into())
      .collect::<Vec<_>>()
  }
}

#[derive(Serialize, Deserialize, Default, Clone, Eq, PartialEq, Debug)]
pub struct ChildView {
  pub id: String,
  pub name: String,
}

impl ChildView {
  pub fn new(id: String) -> Self {
    Self {
      id,
      name: "".to_string(),
    }
  }
  pub fn from_map(map: &HashMap<String, lib0Any>) -> Option<Self> {
    if let lib0Any::String(id) = map.get("id")? {
      if let lib0Any::String(name) = map.get("name")? {
        return Some(Self {
          id: id.to_string(),
          name: name.to_string(),
        });
      }
    }

    None
  }
}

impl From<ChildView> for lib0Any {
  fn from(value: ChildView) -> Self {
    let mut map = HashMap::new();
    map.insert("id".to_string(), lib0Any::String(value.id.into_boxed_str()));
    map.insert(
      "name".to_string(),
      lib0Any::String(value.name.into_boxed_str()),
    );
    lib0Any::Map(Box::new(map))
  }
}
