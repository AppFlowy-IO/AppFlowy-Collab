use serde::{Deserialize, Serialize};

use crate::{SectionsByUid, View, Workspace};

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct FolderData {
  pub uid: i64,
  pub workspace: Workspace,
  pub current_view: String,
  pub views: Vec<View>,
  #[serde(default)]
  pub favorites: SectionsByUid,
  #[serde(default)]
  pub recent: SectionsByUid,
  #[serde(default)]
  pub trash: SectionsByUid,
  #[serde(default)]
  pub private: SectionsByUid,
}

impl FolderData {
  pub fn new(uid: i64, workspace: Workspace) -> Self {
    Self {
      uid,
      workspace,
      current_view: "".to_string(),
      views: vec![],
      favorites: SectionsByUid::new(),
      recent: SectionsByUid::new(),
      trash: SectionsByUid::new(),
      private: SectionsByUid::new(),
    }
  }
}

#[derive(Clone, Debug)]
pub struct TrashInfo {
  pub id: String,
  pub name: String,
  pub created_at: i64,
}
impl AsRef<str> for TrashInfo {
  fn as_ref(&self) -> &str {
    &self.id
  }
}
