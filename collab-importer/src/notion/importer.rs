use crate::error::ImporterError;
use crate::notion::page::NotionView;
use crate::notion::walk_dir::{file_name_from_path, process_entry};
use serde::Serialize;
use std::path::PathBuf;
use walkdir::WalkDir;

#[derive(Debug)]
pub struct NotionImporter {
  host: String,
  workspace_id: String,
  path: PathBuf,
  name: String,
  pub views: Option<NotionView>,
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

  pub async fn import(mut self) -> Result<ImportedView, ImporterError> {
    let views = self.collect_views().await?;
    Ok(ImportedView {
      workspace_id: self.workspace_id,
      host: self.host,
      name: self.name,
      views,
    })
  }

  async fn collect_views(&mut self) -> Result<Vec<NotionView>, ImporterError> {
    let views = WalkDir::new(&self.path)
      .max_depth(1)
      .into_iter()
      .filter_map(|e| e.ok())
      .filter_map(|entry| process_entry(&self.host, &self.workspace_id, &entry))
      .collect::<Vec<NotionView>>();

    Ok(views)
  }
}

#[derive(Debug, Serialize)]
pub struct ImportedView {
  pub workspace_id: String,
  pub host: String,
  pub name: String,
  pub views: Vec<NotionView>,
}

impl ImportedView {
  pub fn upload_files(&self) -> Vec<(String, Vec<PathBuf>)> {
    self
      .views
      .iter()
      .flat_map(|view| view.get_upload_files_recursively())
      .collect()
  }

  pub fn size(&self) -> usize {
    self.views.len()
      + self
        .views
        .iter()
        .map(|view| view.children.len())
        .sum::<usize>()
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
