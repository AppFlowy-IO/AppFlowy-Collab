use std::collections::HashMap;
use std::fmt::{Display, Formatter};

use serde_repr::{Deserialize_repr, Serialize_repr};

/// The type of the collab object. It will be used to determine what kind of services should be
/// used to handle the object.
/// The value of the enum can't be changed.
#[derive(Clone, Debug, Eq, PartialEq, Serialize_repr, Deserialize_repr, Hash)]
#[repr(i32)]
pub enum CollabType {
  Document = 0,
  Database = 1,
  WorkspaceDatabase = 2,
  Folder = 3,
  DatabaseRow = 4,
  UserAwareness = 5,
  Empty = 6,
}

impl CollabType {
  pub fn value(&self) -> i32 {
    self.clone() as i32
  }
}

impl Display for CollabType {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Document => f.write_str("Document"),
      Self::Database => f.write_str("Database"),
      Self::WorkspaceDatabase => f.write_str("WorkspaceDatabase"),
      Self::DatabaseRow => f.write_str("DatabaseRow"),
      Self::Folder => f.write_str("Folder"),
      Self::UserAwareness => f.write_str("UserAwareness"),
      Self::Empty => f.write_str("Empty"),
    }
  }
}

#[derive(Clone, Debug)]
pub struct CollabObject {
  pub object_id: String,
  pub uid: i64,
  pub collab_type: CollabType,
  pub device_id: String,
  pub workspace_id: String,
  pub meta: HashMap<String, String>,
}

impl CollabObject {
  pub fn new(
    uid: i64,
    object_id: String,
    collab_type: CollabType,
    workspace_id: String,
    device_id: String,
  ) -> Self {
    Self {
      object_id,
      uid,
      collab_type,
      workspace_id,
      device_id,
      meta: Default::default(),
    }
  }

  pub fn with_meta(mut self, key: &str, value: String) -> Self {
    self.meta.insert(key.to_string(), value);
    self
  }
}

impl Display for CollabObject {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    f.write_fmt(format_args!("{:?}:{}]", self.collab_type, self.object_id,))
  }
}
