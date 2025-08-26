use super::view::{FolderData, View, ViewPatch, Workspace};
use crate::ViewId;
use crate::v2::folder::FractionalVec;
use crate::v2::fractional_index::FractionalIndex;
use std::collections::{BTreeMap, HashSet};

#[derive(Clone, Debug, Default)]
pub struct FilterOptions {
  pub parent_id: Option<ViewId>,
  pub view_ids: HashSet<ViewId>,
}

#[async_trait::async_trait]
pub trait FolderDataProvider {
  async fn init(&self, data: &FolderData) -> super::Result<()>;

  async fn folder_data(&self, workspace_id: &str) -> super::Result<FolderData>;

  async fn insert_views(&self, views: &[View], uid: i64) -> super::Result<()>;

  async fn delete_views(&self, view_ids: &[ViewId]) -> super::Result<()>;

  async fn update_view(&self, patch: ViewPatch) -> super::Result<View>;
}

struct NoopFolderDataProvider;

#[async_trait::async_trait]
impl FolderDataProvider for NoopFolderDataProvider {
  async fn init(&self, data: &FolderData) -> crate::v2::Result<()> {
    Ok(())
  }

  async fn folder_data(&self, workspace_id: &str) -> crate::v2::Result<FolderData> {
    Ok(FolderData::new(Workspace::new(workspace_id.into())))
  }

  async fn insert_views(&self, views: &[View], uid: i64) -> crate::v2::Result<()> {
    Ok(())
  }

  async fn delete_views(&self, view_ids: &[ViewId]) -> crate::v2::Result<()> {
    Ok(())
  }

  async fn update_view(&self, patch: ViewPatch) -> crate::v2::Result<View> {
    Ok(View::new(patch.id))
  }
}
