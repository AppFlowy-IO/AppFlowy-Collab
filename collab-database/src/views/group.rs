use crate::database::gen_database_group_id;
use crate::{impl_i64_update, impl_str_update};
use collab::core::array_wrapper::ArrayRefExtension;
use collab::preclude::map::MapPrelim;
use collab::preclude::{
  lib0Any, Array, ArrayRef, MapRef, MapRefExtension, ReadTxn, TransactionMut, YrsValue,
};
use serde::{Deserialize, Serialize};

pub struct GroupSettingArray {
  array_ref: ArrayRef,
}

impl GroupSettingArray {
  pub fn new(array_ref: ArrayRef) -> Self {
    Self { array_ref }
  }

  pub fn extends_with_txn(&self, txn: &mut TransactionMut, others: Vec<GroupSetting>) {
    let array_ref = ArrayRefExtension(&self.array_ref);
    for group in others {
      let group_map_ref = array_ref.insert_map_with_txn(txn);
      GroupBuilder::new(&group.id, txn, group_map_ref).update(|update| {
        update
          .set_items(group.groups)
          .set_content(group.content)
          .set_field_type(group.field_type)
          .set_field_id(group.field_id);
      });
    }
  }

  pub fn get_group_setting_with_txn<T: ReadTxn>(&self, txn: &T) -> Vec<GroupSetting> {
    self
      .array_ref
      .iter(txn)
      .flat_map(|v| group_from_value(v, txn))
      .collect::<Vec<GroupSetting>>()
  }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq, Hash)]
pub struct GroupSetting {
  pub id: String,
  pub field_id: String,
  pub field_type: i64,
  pub groups: Vec<Group>,
  pub content: String,
}

impl GroupSetting {
  pub fn new(field_id: String, field_type: i64, content: String) -> Self {
    Self {
      id: gen_database_group_id(),
      field_id,
      field_type,
      groups: vec![],
      content,
    }
  }
}

const GROUP_ID: &str = "id";
const FIELD_ID: &str = "field_id";
const FIELD_TYPE: &str = "ty";
const GROUP_SETTING_GROUPS: &str = "groups";
const GROUP_SETTING_CONTENT: &str = "content";

pub struct GroupBuilder<'a, 'b> {
  id: &'a str,
  map_ref: MapRef,
  txn: &'a mut TransactionMut<'b>,
}

impl<'a, 'b> GroupBuilder<'a, 'b> {
  pub fn new(id: &'a str, txn: &'a mut TransactionMut<'b>, map_ref: MapRef) -> Self {
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

pub struct GroupUpdate<'a, 'b> {
  #[allow(dead_code)]
  id: &'a str,
  map_ref: &'a MapRef,
  txn: &'a mut TransactionMut<'b>,
}

impl<'a, 'b> GroupUpdate<'a, 'b> {
  pub fn new(id: &'a str, txn: &'a mut TransactionMut<'b>, map_ref: &'a MapRef) -> Self {
    Self { id, map_ref, txn }
  }

  impl_str_update!(set_field_id, set_field_id_if_not_none, FIELD_ID);
  impl_str_update!(set_content, set_content_if_not_none, GROUP_SETTING_CONTENT);
  impl_i64_update!(set_field_type, set_field_type_if_not_none, FIELD_TYPE);

  pub fn set_items(self, items: Vec<Group>) -> Self {
    let array_ref = self
      .map_ref
      .get_or_insert_array_with_txn::<MapPrelim<lib0Any>>(self.txn, GROUP_SETTING_GROUPS);
    let items_array = GroupArray::new(array_ref);
    items_array.extends_with_txn(self.txn, items);
    self
  }

  pub fn done(self) -> Option<GroupSetting> {
    group_from_map_ref(self.map_ref, self.txn)
  }
}

pub fn group_from_value<T: ReadTxn>(value: YrsValue, txn: &T) -> Option<GroupSetting> {
  if let YrsValue::YMap(map_ref) = value {
    group_from_map_ref(&map_ref, txn)
  } else {
    None
  }
}

pub fn group_from_map_ref<T: ReadTxn>(map_ref: &MapRef, txn: &T) -> Option<GroupSetting> {
  let id = map_ref.get_str_with_txn(txn, GROUP_ID)?;
  let content = map_ref.get_str_with_txn(txn, GROUP_SETTING_CONTENT)?;
  let field_id = map_ref.get_str_with_txn(txn, FIELD_ID)?;
  let field_type = map_ref.get_i64_with_txn(txn, FIELD_TYPE)?;

  let items = map_ref
    .get_array_ref_with_txn(txn, GROUP_SETTING_GROUPS)
    .map(|array_ref| get_items_with_txn(txn, array_ref))
    .unwrap_or_default();

  Some(GroupSetting {
    id,
    field_id,
    field_type,
    groups: items,
    content,
  })
}

pub struct GroupArray {
  array_ref: ArrayRef,
}

impl GroupArray {
  pub fn new(array_ref: ArrayRef) -> Self {
    Self { array_ref }
  }

  pub fn extends_with_txn(&self, txn: &mut TransactionMut, others: Vec<Group>) {
    let array_ref = ArrayRefExtension(&self.array_ref);
    for items in others {
      let filter_map_ref = array_ref.insert_map_with_txn(txn);
      items.fill_map_ref(txn, filter_map_ref);
    }
  }
}

pub fn get_items_with_txn<T: ReadTxn>(txn: &T, array_ref: ArrayRef) -> Vec<Group> {
  let mut items = vec![];
  array_ref.iter(txn).for_each(|v| {
    if let YrsValue::YMap(map_ref) = v {
      if let Some(item) = Group::from_map_ref(txn, map_ref) {
        items.push(item);
      }
    }
  });
  items
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq, Hash)]
pub struct Group {
  pub id: String,
  pub name: String,
  #[serde(default = "GROUP_REV_VISIBILITY")]
  pub visible: bool,
}

const GROUP_REV_VISIBILITY: fn() -> bool = || true;

impl Group {
  pub fn new(id: String, name: String) -> Self {
    Self {
      id,
      name,
      visible: true,
    }
  }

  pub fn fill_map_ref(self, txn: &mut TransactionMut, map_ref: MapRef) {
    map_ref.insert_with_txn(txn, "id", self.id);
    map_ref.insert_with_txn(txn, "name", self.name);
    map_ref.insert_with_txn(txn, "visible", self.visible);
  }

  pub fn from_map_ref<T: ReadTxn>(txn: &T, map_ref: MapRef) -> Option<Self> {
    let id = map_ref.get_str_with_txn(txn, "id")?;
    let name = map_ref.get_str_with_txn(txn, "name").unwrap_or_default();
    let visible = map_ref
      .get_bool_with_txn(txn, "visible")
      .unwrap_or_default();
    Some(Self { id, name, visible })
  }
}
