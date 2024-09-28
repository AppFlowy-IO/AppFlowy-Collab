use crate::error::ImporterError;
use crate::imported_collab::ImportedCollabInfo;
use crate::notion::file::NotionFile;
use crate::notion::page::{build_imported_collab_recursively, NotionPage};
use crate::notion::walk_dir::{file_name_from_path, process_entry};
use collab_folder::hierarchy_builder::{NestedViews, ParentChildViews, ViewBuilder};
use collab_folder::ViewLayout;
use futures::stream;
use futures::stream::{Stream, StreamExt};
use serde::Serialize;
use std::path::PathBuf;
use std::pin::Pin;
use walkdir::WalkDir;

#[derive(Debug)]
pub struct NotionImporter {
  host: String,
  workspace_id: String,
  path: PathBuf,
  name: String,
  pub views: Option<NotionPage>,
}

impl NotionImporter {
  pub fn new<P: Into<PathBuf>, S: ToString>(
    file_path: P,
    workspace_id: S,
    host: String,
  ) -> Result<Self, ImporterError> {
    let path = file_path.into();
    if !path.exists() {
      return Err(ImporterError::InvalidPath(
        "Path: does not exist".to_string(),
      ));
    }

    let name = file_name_from_path(&path).unwrap_or_else(|_| {
      let now = chrono::Utc::now();
      format!("import-{}", now.format("%Y-%m-%d %H:%M"))
    });

    Ok(Self {
      host,
      workspace_id: workspace_id.to_string(),
      path,
      name,
      views: None,
    })
  }

  /// Return a ImportedInfo struct that contains all the views and their children recursively.
  pub async fn import(mut self) -> Result<ImportedInfo, ImporterError> {
    let views = self.collect_views().await?;
    Ok(ImportedInfo {
      workspace_id: self.workspace_id,
      host: self.host,
      name: self.name,
      views,
    })
  }

  async fn collect_views(&mut self) -> Result<Vec<NotionPage>, ImporterError> {
    let views = WalkDir::new(&self.path)
      .max_depth(1)
      .into_iter()
      .filter_map(|e| e.ok())
      .filter_map(|entry| process_entry(&self.host, &self.workspace_id, &entry))
      .collect::<Vec<NotionPage>>();
    Ok(views)
  }
}

#[derive(Debug, Serialize)]
pub struct ImportedInfo {
  pub workspace_id: String,
  pub host: String,
  pub name: String,
  pub views: Vec<NotionPage>,
}

pub type ImportedCollabInfoStream<'a> = Pin<Box<dyn Stream<Item = ImportedCollabInfo> + 'a>>;
impl ImportedInfo {
  pub async fn into_collab_stream(self) -> ImportedCollabInfoStream<'static> {
    // Create a stream for each view by resolving the futures into streams
    let view_streams = self
      .views
      .into_iter()
      .map(|view| async { build_imported_collab_recursively(view).await });

    let combined_stream = stream::iter(view_streams)
      .then(|stream_future| stream_future)
      .flatten();

    Box::pin(combined_stream)
  }

  pub async fn build_nested_views(&self, uid: i64) -> NestedViews {
    let views = stream::iter(&self.views)
      .then(|notion_page| convert_notion_page_to_parent_child(&self.workspace_id, notion_page, uid))
      .collect()
      .await;
    NestedViews { views }
  }

  pub fn num_of_csv(&self) -> usize {
    self
      .views
      .iter()
      .map(|view| view.num_of_csv())
      .sum::<usize>()
  }

  pub fn num_of_markdown(&self) -> usize {
    self
      .views
      .iter()
      .map(|view| view.num_of_markdown())
      .sum::<usize>()
  }
}

#[async_recursion::async_recursion]
async fn convert_notion_page_to_parent_child(
  parent_id: &str,
  notion_page: &NotionPage,
  uid: i64,
) -> ParentChildViews {
  let view_layout = match notion_page.notion_file {
    NotionFile::Unknown => ViewLayout::Document,
    NotionFile::CSV { .. } => ViewLayout::Grid,
    NotionFile::CSVPart { .. } => ViewLayout::Grid,
    NotionFile::Markdown { .. } => ViewLayout::Document,
  };
  let mut view_builder = ViewBuilder::new(uid, parent_id.to_string())
    .with_name(&notion_page.notion_name)
    .with_layout(view_layout)
    .with_view_id(&notion_page.view_id);

  for child_notion_page in &notion_page.children {
    view_builder = view_builder
      .with_child_view_builder(|_| async {
        convert_notion_page_to_parent_child(&notion_page.view_id, child_notion_page, uid).await
      })
      .await;
  }

  view_builder.build()
}
