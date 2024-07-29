use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct DocumentAwarenessState {
  // the fields supported in version 1 contain the user, selection, metadata, and timestamp fields
  pub version: i64,
  pub user: DocumentAwarenessUser,
  pub selection: Option<DocumentAwarenessSelection>,
  // The `metadata` field is an optional field (json string) that can be used to store additional information.
  // For example, the user can store the color of the selection in this field
  pub metadata: Option<String>,
  pub timestamp: i64,
}

impl DocumentAwarenessState {
  pub fn new(version: i64, user: DocumentAwarenessUser) -> Self {
    Self {
      version,
      user,
      selection: None,
      metadata: None,
      timestamp: 0,
    }
  }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct DocumentAwarenessUser {
  pub uid: i64,
  pub device_id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct DocumentAwarenessSelection {
  pub start: DocumentAwarenessPosition,
  pub end: DocumentAwarenessPosition,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct DocumentAwarenessPosition {
  pub path: Vec<u64>,
  pub offset: u64,
}
