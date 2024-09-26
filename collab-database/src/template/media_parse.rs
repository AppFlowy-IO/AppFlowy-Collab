use crate::fields::media_type_option::{MediaCellData, MediaFile, MediaFileType, MediaUploadType};
use crate::template::builder::FileUrlBuilder;
use crate::template::csv::CSVResource;
use futures::stream::{FuturesOrdered, StreamExt};

use std::path::PathBuf;

use tokio::fs::metadata;

pub(crate) async fn replace_cells_with_files(
  cells: Vec<String>,
  database_id: &str,
  csv_resource: &Option<CSVResource>,
  file_url_builder: &Option<Box<dyn FileUrlBuilder>>,
) -> Vec<Option<MediaCellData>> {
  match csv_resource {
    None => vec![],
    Some(csv_resource) => {
      let mut futures = FuturesOrdered::new();
      for cell in cells {
        futures.push_back(async move {
          if cell.is_empty() {
            None
          } else {
            let files = futures::stream::iter(cell.split(','))
              .filter_map(|file| {
                let path = csv_resource
                  .files
                  .iter()
                  .find(|resource| resource.ends_with(file))
                  .map(PathBuf::from);

                async move {
                  let path = path?;
                  if metadata(&path).await.is_ok() {
                    let file_name = path
                      .file_name()
                      .unwrap_or_default()
                      .to_string_lossy()
                      .to_string();
                    let url = file_url_builder.as_ref()?.build(database_id, &path).await?;
                    let media_type = MediaFileType::from_file(&path);

                    Some(MediaFile::new(
                      file_name,
                      url,
                      MediaUploadType::Cloud,
                      media_type,
                    ))
                  } else {
                    None
                  }
                }
              })
              .collect::<Vec<_>>()
              .await;
            Some(MediaCellData { files })
          }
        });
      }

      futures.collect().await
    },
  }
}
