use super::view::{FolderState, ViewData, ViewPatch};
use crate::ViewId;
use std::collections::HashSet;

#[derive(Clone, Debug, Default)]
pub struct FilterOptions {
  pub parent_id: Option<ViewId>,
  pub view_ids: HashSet<ViewId>,
}

#[async_trait::async_trait]
pub trait FolderDataProvider {
  async fn init(&self, data: &FolderState) -> super::Result<()>;

  async fn folder_data(&self, workspace_id: &str) -> super::Result<FolderState>;

  async fn insert_views(&self, views: &[ViewData], uid: i64) -> super::Result<()>;

  async fn delete_views(&self, view_ids: &[ViewId]) -> super::Result<()>;

  async fn update_view(&self, patch: ViewPatch) -> super::Result<()>;
}

pub struct NoopFolderDataProvider;

#[async_trait::async_trait]
impl FolderDataProvider for NoopFolderDataProvider {
  async fn init(&self, data: &FolderState) -> crate::v2::Result<()> {
    Ok(())
  }

  async fn folder_data(&self, workspace_id: &str) -> crate::v2::Result<FolderState> {
    Ok(FolderState::new(workspace_id.into()))
  }

  async fn insert_views(&self, views: &[ViewData], uid: i64) -> crate::v2::Result<()> {
    Ok(())
  }

  async fn delete_views(&self, view_ids: &[ViewId]) -> crate::v2::Result<()> {
    Ok(())
  }

  async fn update_view(&self, patch: ViewPatch) -> crate::v2::Result<()> {
    Ok(())
  }
}
