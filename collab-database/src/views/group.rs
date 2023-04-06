use crate::database::gen_database_group_id;
use crate::{impl_i64_update, impl_str_update};
use collab::core::any_map::{AnyMap, AnyMapBuilder};
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
      GroupSettingBuilder::new(&group.id, txn, group_map_ref).update(|update| {
        update
          .set_groups(group.groups)
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
      .flat_map(|v| group_setting_from_value(v, txn))
      .collect::<Vec<GroupSetting>>()
  }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GroupSetting {
  pub id: String,
  pub field_id: String,
  pub field_type: i64,
  pub groups: Vec<GroupMap>,
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
const GROUPS: &str = "groups";
const CONTENT: &str = "content";

pub struct GroupSettingBuilder<'a, 'b> {
  id: &'a str,
  map_ref: MapRef,
  txn: &'a mut TransactionMut<'b>,
}

impl<'a, 'b> GroupSettingBuilder<'a, 'b> {
  pub fn new(id: &'a str, txn: &'a mut TransactionMut<'b>, map_ref: MapRef) -> Self {
    map_ref.insert_with_txn(txn, GROUP_ID, id);
    Self { id, map_ref, txn }
  }

  pub fn update<F>(self, f: F) -> Self
  where
    F: FnOnce(GroupSettingUpdate),
  {
    let update = GroupSettingUpdate::new(self.id, self.txn, &self.map_ref);
    f(update);
    self
  }
  pub fn done(self) {}
}

pub struct GroupSettingUpdate<'a, 'b> {
  #[allow(dead_code)]
  id: &'a str,
  map_ref: &'a MapRef,
  txn: &'a mut TransactionMut<'b>,
}

impl<'a, 'b> GroupSettingUpdate<'a, 'b> {
  pub fn new(id: &'a str, txn: &'a mut TransactionMut<'b>, map_ref: &'a MapRef) -> Self {
    Self { id, map_ref, txn }
  }

  impl_str_update!(set_field_id, set_field_id_if_not_none, FIELD_ID);
  impl_str_update!(set_content, set_content_if_not_none, CONTENT);
  impl_i64_update!(set_field_type, set_field_type_if_not_none, FIELD_TYPE);

  pub fn set_groups(self, items: Vec<GroupMap>) -> Self {
    let array_ref = self
      .map_ref
      .get_or_insert_array_with_txn::<MapPrelim<lib0Any>>(self.txn, GROUPS);
    let items_array = GroupArray::new(array_ref);
    items_array.extends_with_txn(self.txn, items);
    self
  }

  pub fn done(self) -> Option<GroupSetting> {
    group_setting_from_map_ref(self.map_ref, self.txn)
  }
}

pub fn group_setting_from_value<T: ReadTxn>(value: YrsValue, txn: &T) -> Option<GroupSetting> {
  if let YrsValue::YMap(map_ref) = value {
    group_setting_from_map_ref(&map_ref, txn)
  } else {
    None
  }
}

pub fn group_setting_from_map_ref<T: ReadTxn>(map_ref: &MapRef, txn: &T) -> Option<GroupSetting> {
  let id = map_ref.get_str_with_txn(txn, GROUP_ID)?;
  let content = map_ref.get_str_with_txn(txn, CONTENT)?;
  let field_id = map_ref.get_str_with_txn(txn, FIELD_ID)?;
  let field_type = map_ref.get_i64_with_txn(txn, FIELD_TYPE)?;

  let groups = map_ref
    .get_array_ref_with_txn(txn, GROUPS)
    .map(|array_ref| get_groups_with_txn(txn, array_ref))
    .unwrap_or_default();

  Some(GroupSetting {
    id,
    field_id,
    field_type,
    groups,
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

  pub fn extends_with_txn(&self, txn: &mut TransactionMut, others: Vec<GroupMap>) {
    let array_ref = ArrayRefExtension(&self.array_ref);
    for items in others {
      let filter_map_ref = array_ref.insert_map_with_txn(txn);
      items.fill_map_ref(txn, filter_map_ref);
    }
  }
}

pub fn get_groups_with_txn<T: ReadTxn>(txn: &T, array_ref: ArrayRef) -> Vec<GroupMap> {
  let mut items = vec![];
  array_ref.iter(txn).for_each(|v| {
    if let YrsValue::YMap(map_ref) = v {
      items.push(GroupMap::from_map_ref(txn, map_ref));
    }
  });
  items
}

pub type GroupMap = AnyMap;
pub type GroupMapBuilder = AnyMapBuilder;
