use serde::{Deserialize, Serialize};

use crate::{RepeatedViewIdentifier, View, ViewLayout};

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct Workspace {
  pub id: String,
  pub name: String,
  pub child_views: RepeatedViewIdentifier,
  pub created_at: i64,
}

impl Workspace {
  pub fn new(id: String, name: String) -> Self {
    debug_assert!(!id.is_empty());
    Self {
      id,
      name,
      child_views: Default::default(),
      created_at: chrono::Utc::now().timestamp(),
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
    }
  }
}
impl From<Workspace> for View {
  fn from(value: Workspace) -> Self {
    Self {
      id: value.id,
      parent_view_id: "".to_string(),
      name: value.name,
      desc: "".to_string(),
      children: value.child_views,
      created_at: value.created_at,
      is_favorite: false,
      layout: ViewLayout::Document,
      icon: None,
    }
  }
}
