use crate::error::ImporterError;
use crate::notion::NotionImporter;
use crate::util::unzip;
use collab::entity::EncodedCollab;
use collab_entity::CollabType;
use std::env::temp_dir;
use std::fmt::Display;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct RepeatedImportedCollabInfo {
  pub infos: Vec<ImportedCollabInfo>,
  pub total_size: u64,
}

impl Display for RepeatedImportedCollabInfo {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    for info in &self.infos {
      write!(f, "{}\n\n", info)?;
    }
    write!(f, "Total size: {}", self.total_size)
  }
}

#[derive(Debug, Clone)]
pub struct ImportedCollabInfo {
  pub name: String,
  pub collabs: Vec<ImportedCollab>,
  /// All files that related to current collab
  pub files: Vec<String>,
  /// The total payload size for current collab and its files
  pub file_size: u64,
  pub import_type: ImportType,
}

#[derive(Debug, Clone)]
pub enum ImportType {
  Database,
  Document,
}

impl Display for ImportType {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      ImportType::Database => write!(f, "Database"),
      ImportType::Document => write!(f, "Document"),
    }
  }
}

impl Display for ImportedCollabInfo {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    let file_paths: String = self.files.join(", ");

    write!(
      f,
      "{}:{} - {} collabs, {} files, {} bytes\nFiles: [{}]",
      self.name,
      self.import_type,
      self.collabs.len(),
      self.files.len(),
      self.file_size,
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

pub async fn import_notion_zip_file(
  host: &str,
  workspace_id: &str,
  zip_file: PathBuf,
) -> Result<RepeatedImportedCollabInfo, ImporterError> {
  let unzip_file = unzip(zip_file, temp_dir())?;
  let imported = NotionImporter::new(&unzip_file, workspace_id, host.to_string())?
    .import()
    .await?;

  let total_size = imported.all_file_size() as u64;
  let infos = imported.all_imported_collabs().await;
  Ok(RepeatedImportedCollabInfo { infos, total_size })
}
