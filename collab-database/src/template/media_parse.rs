use crate::fields::media_type_option::{MediaCellData, MediaFile, MediaFileType, MediaUploadType};
use crate::util::{upload_file_url, DatabaseFileId};
use rayon::prelude::*;
use std::path::PathBuf;

pub(crate) async fn replace_cells_with_files(
  server_url: &Option<String>,
  workspace_id: &str,
  cells: Vec<String>,
  database_id: &str,
  resources: &[String],
) -> Vec<Option<MediaCellData>> {
  match server_url {
    None => vec![],
    Some(host) => cells
      .into_par_iter()
      .map(|cell| {
        if cell.is_empty() {
          return None;
        }
        let files = cell
          .split(", ")
          .par_bridge()
          .filter_map(|file| {
            let resource = resources.iter().find(|resource| resource.ends_with(file))?;
            let path = PathBuf::from(resource);
            if path.exists() {
              let file_name = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
              let file_id = DatabaseFileId::from_path(&path).ok()?;
              let url = upload_file_url(host, workspace_id, database_id, &file_id);
              let media_type = MediaFileType::from_file(path);
              Some(MediaFile::new(
                file_name,
                url,
                MediaUploadType::Cloud,
                media_type,
              ))
            } else {
              None
            }
          })
          .collect::<Vec<_>>();

        Some(MediaCellData { files })
      })
      .collect(),
  }
}
