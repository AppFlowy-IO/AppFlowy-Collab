use serde::{Deserialize, Serialize};

use crate::{FavoritesByUid, View, Workspace};

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct FolderData {
  pub workspace: Workspace,
  pub current_view: String,
  pub views: Vec<View>,
  #[serde(default)]
  pub favorites: FavoritesByUid,
}

impl FolderData {
  pub fn new(workspace: Workspace) -> Self {
    Self {
      workspace,
      current_view: "".to_string(),
      views: vec![],
      favorites: FavoritesByUid::new(),
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
