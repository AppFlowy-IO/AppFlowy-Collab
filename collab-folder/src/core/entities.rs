use serde::{Deserialize, Serialize};

use crate::core::{View, Workspace};

#[derive(Serialize, Deserialize)]
pub struct FolderData {
  pub current_workspace_id: String,
  pub current_view: String,
  pub workspaces: Vec<Workspace>,
  /// All views in the folder.
  pub views: Vec<View>,
}

#[derive(Clone)]
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

#[derive(Clone)]
pub struct FavoritesInfo {
  pub id: String,
}
impl AsRef<str> for FavoritesInfo {
  fn as_ref(&self) -> &str {
    &self.id
  }
}
