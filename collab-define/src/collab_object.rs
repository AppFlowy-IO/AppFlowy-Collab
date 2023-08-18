use std::collections::HashMap;
use std::fmt::{Display, Formatter};

/// The type of the collab object. It will be used to determine what kind of services should be
/// used to handle the object.
/// The value of the enum can't be changed.
#[derive(Clone, Debug)]
pub enum CollabType {
  Document = 0,
  Database = 1,
  WorkspaceDatabase = 2,
  Folder = 3,
  DatabaseRow = 4,
  UserAwareness = 5,
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
    }
  }
}

#[derive(Clone, Debug)]
pub struct CollabObject {
  pub object_id: String,
  pub uid: i64,
  pub ty: CollabType,
  pub meta: HashMap<String, String>,
}

impl CollabObject {
  pub fn new(uid: i64, object_id: String, ty: CollabType) -> Self {
    Self {
      object_id,
      uid,
      ty,
      meta: Default::default(),
    }
  }

  pub fn with_device_id(mut self, device_id: String) -> Self {
    self.meta.insert("device_id".to_string(), device_id);
    self
  }

  pub fn with_workspace_id(mut self, workspace_id: String) -> Self {
    self.meta.insert("workspace_id".to_string(), workspace_id);
    self
  }

  pub fn with_meta(mut self, key: &str, value: String) -> Self {
    self.meta.insert(key.to_string(), value);
    self
  }

  pub fn get_workspace_id(&self) -> Option<String> {
    self.meta.get("workspace_id").cloned()
  }

  pub fn get_device_id(&self) -> String {
    match self.meta.get("device_id").cloned() {
      None => uuid::Uuid::new_v4().to_string(),
      Some(device_id) => device_id,
    }
  }
}

impl Display for CollabObject {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    f.write_fmt(format_args!("{:?}:{}]", self.ty, self.object_id,))
  }
}
