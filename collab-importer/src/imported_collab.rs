use crate::error::ImporterError;
use crate::notion::NotionImporter;
use crate::util::unzip;
use collab::entity::EncodedCollab;
use collab_entity::CollabType;
use std::env::temp_dir;
use std::path::PathBuf;

pub struct ImportedCollabInfo {
  pub name: String,
  pub collabs: Vec<ImportedCollab>,
  pub files: Vec<PathBuf>,
  pub file_size: u64,
}

pub struct ImportedCollab {
  pub object_id: String,
  pub collab_type: CollabType,
  pub encoded_collab: EncodedCollab,
}

pub async fn import_notion_zip_file(
  host: &str,
  workspace_id: &str,
  zip_file: PathBuf,
) -> Result<Vec<ImportedCollabInfo>, ImporterError> {
  let unzip_file = unzip(zip_file, temp_dir())?;
  let imported = NotionImporter::new(&unzip_file, workspace_id, host.to_string())?
    .import()
    .await?;

  let imported_collab_info_list = imported.all_imported_collabs().await;
  Ok(imported_collab_info_list)
}
