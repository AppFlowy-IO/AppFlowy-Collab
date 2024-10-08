use crate::error::ImporterError;
use crate::notion::page::CollabResource;
use crate::notion::NotionImporter;
use crate::util::{unzip_from_path_or_memory, Either};
use collab::entity::EncodedCollab;
use collab_entity::CollabType;

use futures::StreamExt;
use std::fmt::Display;
use std::ops::{Deref, DerefMut};
use std::path::PathBuf;

pub async fn import_notion_zip_file(
  uid: i64,
  host: &str,
  workspace_id: &str,
  zip_file: PathBuf,
  output_dir: PathBuf,
) -> Result<RepeatedImportedCollabInfo, ImporterError> {
  if !zip_file.exists() {
    return Err(ImporterError::FileNotFound);
  }

  let unzip_file = unzip_from_path_or_memory(Either::Left(zip_file), output_dir).await?;
  let imported = NotionImporter::new(uid, &unzip_file, workspace_id, host.to_string())?
    .import()
    .await?;

  let infos = imported
    .into_collab_stream()
    .await
    .collect::<Vec<ImportedCollabInfo>>()
    .await;
  Ok(RepeatedImportedCollabInfo { infos })
}

#[derive(Debug, Clone)]
pub struct RepeatedImportedCollabInfo {
  pub infos: Vec<ImportedCollabInfo>,
}

impl Deref for RepeatedImportedCollabInfo {
  type Target = Vec<ImportedCollabInfo>;

  fn deref(&self) -> &Self::Target {
    &self.infos
  }
}

impl DerefMut for RepeatedImportedCollabInfo {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.infos
  }
}

impl RepeatedImportedCollabInfo {
  pub fn size(&self) -> u64 {
    self.infos.iter().map(|i| i.total_size()).sum()
  }
}

impl Display for RepeatedImportedCollabInfo {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    for info in &self.infos {
      write!(f, "{}\n\n", info)?;
    }
    write!(f, "Total size: {}", self.size())
  }
}

#[derive(Debug, Clone)]
pub struct ImportedCollabInfo {
  pub name: String,
  pub collabs: Vec<ImportedCollab>,
  pub resource: CollabResource,
  pub import_type: ImportType,
}

impl ImportedCollabInfo {
  pub fn total_size(&self) -> u64 {
    let collab_size: u64 = self
      .collabs
      .iter()
      .map(|c| c.encoded_collab.doc_state.len() as u64)
      .sum();

    self.file_size() + collab_size
  }

  pub fn file_size(&self) -> u64 {
    self
      .resource
      .files
      .iter()
      .map(|p| std::fs::metadata(p).map(|m| m.len()).unwrap_or(0))
      .sum()
  }
}

#[derive(Debug, Clone)]
pub enum ImportType {
  Database {
    database_id: String,
    view_ids: Vec<String>,
  },
  Document,
}

impl Display for ImportType {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      ImportType::Database { .. } => write!(f, "Database"),
      ImportType::Document => write!(f, "Document"),
    }
  }
}

impl Display for ImportedCollabInfo {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    let file_paths: String = self.resource.files.join(", ");

    write!(
      f,
      "{}:{} - {} collabs, {} files, {} bytes\nFiles: [{}]",
      self.name,
      self.import_type,
      self.collabs.len(),
      self.resource.files.len(),
      self.total_size(),
      file_paths
    )
  }
}

#[derive(Debug, Clone)]
pub struct ImportedCollab {
  pub object_id: String,
  pub collab_type: CollabType,
  pub encoded_collab: EncodedCollab,
}

impl Display for ImportedCollab {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "Collab: {} - {}", self.object_id, self.collab_type)
  }
}
