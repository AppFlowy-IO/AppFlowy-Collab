use crate::fields::FieldType;
use crate::{impl_any_update, impl_str_update};
use anyhow::bail;
use collab::preclude::map::MapPrelim;
use collab::preclude::{
  lib0Any, Array, ArrayRef, ArrayRefWrapper, Map, MapRef, MapRefTool, MapRefWrapper, ReadTxn,
  TransactionMut, YrsValue,
};
use serde::{Deserialize, Serialize};

pub struct GroupArray {
  array_ref: ArrayRefWrapper,
}

impl GroupArray {
  pub fn new(array_ref: ArrayRefWrapper) -> Self {
    Self { array_ref }
  }

  pub fn extends_with_txn(&self, txn: &mut TransactionMut, others: Vec<Group>) {
    for group in others {
      let group_map_ref = self.array_ref.insert_map_with_txn(txn);
      GroupBuilder::new(&group.id, txn, group_map_ref).update(|update| {
        update
          .set_items(group.items)
          .set_content(group.content)
          .set_field_type(group.field_type)
          .set_field_id(group.field_id);
      });
    }
  }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq, Hash)]
pub struct Group {
  pub id: String,
  pub field_id: String,
  pub field_type: FieldType,
  pub items: Vec<GroupItem>,
  pub content: String,
}
const GROUP_ID: &str = "id";
const FIELD_ID: &str = "field_id";
const FIELD_TYPE: &str = "ty";
const GROUP_ITEMS: &str = "items";
const GROUP_CONTENT: &str = "content";

pub struct GroupBuilder<'a, 'b> {
  id: &'a str,
  map_ref: MapRefWrapper,
  txn: &'a mut TransactionMut<'b>,
}

impl<'a, 'b> GroupBuilder<'a, 'b> {
  pub fn new(id: &'a str, txn: &'a mut TransactionMut<'b>, map_ref: MapRefWrapper) -> Self {
    map_ref.insert_with_txn(txn, GROUP_ID, id);
    Self { id, map_ref, txn }
  }

  pub fn update<F>(self, f: F) -> Self
  where
    F: FnOnce(GroupUpdate),
  {
    let update = GroupUpdate::new(self.id, self.txn, &self.map_ref);
    f(update);
    self
  }
  pub fn done(self) {}
}

pub struct GroupUpdate<'a, 'b, 'c> {
  id: &'a str,
  map_ref: &'c MapRefWrapper,
  txn: &'a mut TransactionMut<'b>,
}

impl<'a, 'b, 'c> GroupUpdate<'a, 'b, 'c> {
  pub fn new(id: &'a str, txn: &'a mut TransactionMut<'b>, map_ref: &'c MapRefWrapper) -> Self {
    Self { id, map_ref, txn }
  }

  impl_str_update!(set_field_id, set_field_id_if_not_none, FIELD_ID);
  impl_str_update!(set_content, set_content_if_not_none, GROUP_CONTENT);
  impl_any_update!(
    set_field_type,
    set_field_type_if_not_none,
    FIELD_TYPE,
    FieldType
  );

  pub fn set_items(self, items: Vec<GroupItem>) -> Self {
    let array_ref = self
      .map_ref
      .get_or_insert_array_with_txn::<MapPrelim<lib0Any>>(self.txn, GROUP_ITEMS);
    let items_array = GroupItemArray::new(array_ref);
    items_array.extends_with_txn(self.txn, items);
    self
  }

  pub fn done(self) -> Option<Group> {
    group_from_map_ref(self.map_ref, self.txn)
  }
}

pub fn group_from_map_ref<T: ReadTxn>(map_ref: &MapRef, txn: &T) -> Option<Group> {
  let map_ref = MapRefTool(map_ref);
  let id = map_ref.get_str_with_txn(txn, GROUP_ID)?;
  let content = map_ref.get_str_with_txn(txn, GROUP_CONTENT)?;
  let field_id = map_ref.get_str_with_txn(txn, FIELD_ID)?;
  let field_type = map_ref
    .get_i64_with_txn(txn, FIELD_TYPE)
    .map(|value| value.try_into().ok())??;

  let items = map_ref
    .get_array_ref_with_txn(txn, GROUP_ITEMS)
    .map(|array_ref| get_items_with_txn(txn, array_ref))
    .unwrap_or_default();

  Some(Group {
    id,
    field_id,
    field_type,
    items,
    content,
  })
}

pub struct GroupItemArray {
  array_ref: ArrayRefWrapper,
}

impl GroupItemArray {
  pub fn new(array_ref: ArrayRefWrapper) -> Self {
    Self { array_ref }
  }

  pub fn extends_with_txn(&self, txn: &mut TransactionMut, others: Vec<GroupItem>) {
    for items in others {
      let filter_map_ref = self.array_ref.insert_map_with_txn(txn);
      items.fill_map_ref(txn, filter_map_ref);
    }
  }
}

pub fn get_items_with_txn<T: ReadTxn>(txn: &T, array_ref: ArrayRef) -> Vec<GroupItem> {
  let mut items = vec![];
  array_ref.iter(txn).for_each(|v| {
    if let YrsValue::YMap(map_ref) = v {
      if let Some(item) = GroupItem::from_map_ref(txn, map_ref) {
        items.push(item);
      }
    }
  });
  items
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq, Hash)]
pub struct GroupItem {
  pub id: String,
  pub name: String,
  #[serde(default = "GROUP_REV_VISIBILITY")]
  pub visible: bool,
}
const GROUP_REV_VISIBILITY: fn() -> bool = || true;

impl GroupItem {
  pub fn fill_map_ref(self, txn: &mut TransactionMut, map_ref: MapRefWrapper) {
    map_ref.insert_with_txn(txn, "id", self.id);
    map_ref.insert_with_txn(txn, "name", self.name);
    map_ref.insert_with_txn(txn, "visible", self.visible);
  }

  pub fn from_map_ref<T: ReadTxn>(txn: &T, map_ref: MapRef) -> Option<Self> {
    let map_ref = MapRefTool(&map_ref);
    let id = map_ref.get_str_with_txn(txn, "id")?;
    let name = map_ref.get_str_with_txn(txn, "name").unwrap_or_default();
    let visible = map_ref
      .get_bool_with_txn(txn, "visible")
      .unwrap_or_default();
    Some(Self { id, name, visible })
  }
}
