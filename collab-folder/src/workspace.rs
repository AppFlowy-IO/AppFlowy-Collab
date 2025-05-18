use serde::{Deserialize, Serialize};

use crate::{RepeatedViewIdentifier, View, ViewLayout, timestamp};

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct Workspace {
  pub id: String,
  pub name: String,
  pub child_views: RepeatedViewIdentifier,
  pub created_at: i64,
  pub created_by: Option<i64>,
  pub last_edited_time: i64,
  pub last_edited_by: Option<i64>,
}

impl Workspace {
  pub fn new(id: String, name: String, uid: i64) -> Self {
    debug_assert!(!id.is_empty());
    let time = timestamp();
    Self {
      id,
      name,
      child_views: Default::default(),
      created_at: time,
      last_edited_time: time,
      created_by: Some(uid),
      last_edited_by: Some(uid),
    }
  }
}

impl From<&View> for Workspace {
  fn from(value: &View) -> Self {
    Self {
      id: value.id.clone(),
      name: value.name.clone(),
      child_views: value.children.clone(),
      created_at: value.created_at,
      created_by: value.created_by,
      last_edited_time: value.last_edited_time,
      last_edited_by: value.last_edited_by,
    }
  }
}
impl From<Workspace> for View {
  fn from(value: Workspace) -> Self {
    Self {
      id: value.id,
      parent_view_id: "".to_string(),
      name: value.name,
      children: value.child_views,
      created_at: value.created_at,
      is_favorite: false,
      layout: ViewLayout::Document,
      icon: None,
      created_by: value.created_by,
      last_edited_time: value.last_edited_time,
      last_edited_by: value.last_edited_by,
      is_locked: None,
      extra: None,
    }
  }
}
