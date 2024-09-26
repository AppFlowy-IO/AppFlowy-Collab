use crate::error::ImporterError;
use crate::imported_collab::ImportedCollabInfo;
use crate::notion::page::NotionPage;
use crate::notion::walk_dir::{file_name_from_path, process_entry};
use futures::stream::{self, Stream, StreamExt};
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

impl ImportedInfo {
  pub async fn all_imported_collabs(&self) -> Pin<Box<dyn Stream<Item = ImportedCollabInfo> + '_>> {
    // Create a stream for each view by resolving the futures into streams
    let view_streams = self
      .views
      .iter()
      .map(|view| async { view.build_imported_collab_recursively().await });

    let combined_stream = stream::iter(view_streams)
      .then(|stream_future| stream_future)
      .flatten();

    Box::pin(combined_stream)
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
