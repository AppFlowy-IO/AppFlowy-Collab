use std::{collections::HashMap, sync::Arc};

use collab::preclude::{Any, ArrayRef};
use serde::{Deserialize, Serialize};
use yrs::encoding::serde::{from_any, to_any};

use crate::database::gen_database_group_id;

/// [GroupSettingArray] contains list of [GroupSettingMap]
pub type GroupSettingArray = Vec<Any>;
pub type GroupSettingArrayUpdate = ArrayRef;

/// [GroupSettingMap] contains list of key/value.
/// One of the key/value represents as the [GroupMap]
pub type GroupSettingMap = HashMap<String, Any>;
pub type GroupSettingBuilder = HashMap<String, Any>;

/// [GroupMap] contains the key/value that represents a group data.
pub type GroupMap = HashMap<String, Any>;
/// [GroupMapBuilder] is the builder for [GroupMap]
pub type GroupMapBuilder = HashMap<String, Any>;

#[derive(Debug, Clone, Default, Deserialize)]
pub struct GroupSetting {
  pub id: String,
  pub field_id: String,
  #[serde(rename = "ty")]
  pub field_type: i64,
  #[serde(default)]
  pub groups: Vec<Group>,
  #[serde(default)]
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

impl TryFrom<GroupSettingMap> for GroupSetting {
  type Error = anyhow::Error;

  fn try_from(value: GroupSettingMap) -> Result<Self, Self::Error> {
    from_any(&Any::from(value)).map_err(|e| e.into())
  }
}

impl From<GroupSetting> for GroupSettingMap {
  fn from(setting: GroupSetting) -> Self {
    let groups = to_any(&setting.groups).unwrap_or_else(|_| Any::Array(Arc::from([])));
    GroupSettingBuilder::from([
      (GROUP_ID.into(), setting.id.into()),
      (FIELD_ID.into(), setting.field_id.into()),
      (FIELD_TYPE.into(), Any::BigInt(setting.field_type)),
      (GROUPS.into(), groups),
      (CONTENT.into(), setting.content.into()),
    ])
  }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Group {
  pub id: String,
  #[serde(default = "GROUP_VISIBILITY")]
  pub visible: bool,
}

impl TryFrom<GroupMap> for Group {
  type Error = anyhow::Error;

  fn try_from(value: GroupMap) -> Result<Self, Self::Error> {
    from_any(&Any::from(value)).map_err(|e| e.into())
  }
}

impl From<Group> for GroupMap {
  fn from(group: Group) -> Self {
    GroupMapBuilder::from([
      ("id".into(), group.id.into()),
      ("visible".into(), group.visible.into()),
    ])
  }
}

const GROUP_VISIBILITY: fn() -> bool = || true;

impl Group {
  pub fn new(id: String) -> Self {
    Self { id, visible: true }
  }
}
