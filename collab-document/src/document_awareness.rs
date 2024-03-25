use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct DocumentAwarenessState {
  pub user: DocumentAwarenessUser,
  pub selection: Option<DocumentAwarenessSelection>,
}

impl DocumentAwarenessState {
  pub fn new(user: DocumentAwarenessUser) -> Self {
    Self {
      user,
      selection: None,
    }
  }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct DocumentAwarenessUser {
  pub uid: i64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct DocumentAwarenessSelection {
  pub start: DocumentAwarenessPosition,
  pub end: DocumentAwarenessPosition,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct DocumentAwarenessPosition {
  pub path: Vec<u64>,
  pub offset: u64,
}
