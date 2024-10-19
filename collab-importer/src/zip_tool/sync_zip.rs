use crate::error::ImporterError;
use crate::zip_tool::async_zip::unzip_single_file;
use crate::zip_tool::util::{is_multi_part_zip_signature, remove_part_suffix};
use anyhow::{anyhow, Result};
use async_recursion::async_recursion;
use std::io;
use std::io::Read;
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;

use zip::read::ZipArchive;

pub struct UnzipFile {
  pub file_name: String,
  pub unzip_dir_path: PathBuf,
  pub parts: Vec<PathBuf>,
}

#[async_recursion(?Send)]
pub async fn sync_unzip(
  file_path: PathBuf,
  out_dir: PathBuf,
  default_file_name: Option<String>,
) -> Result<UnzipFile, ImporterError> {
  // Open the zip file and read it synchronously in a blocking task
  let file = std::fs::File::open(&file_path)
    .map_err(|e| ImporterError::Internal(anyhow!("Failed to open zip file: {:?}", e)))?;

  let mut archive = ZipArchive::new(file)
    .map_err(|e| ImporterError::Internal(anyhow!("Failed to read zip archive: {:?}", e)))?;

  let mut root_dir = None;
  let mut parts = vec![];

  // Iterate through each file in the archive
  for i in 0..archive.len() {
    let mut entry = archive
      .by_index(i)
      .map_err(|e| ImporterError::Internal(anyhow!("Failed to read entry: {:?}", e)))?;

    let filename = entry.name().to_string();
    if root_dir.is_none() && entry.is_dir() {
      root_dir = Some(filename.split('/').next().unwrap_or(&filename).to_string());
    }

    let output_path = out_dir.join(&filename);
    if entry.is_dir() {
      fs::create_dir_all(&output_path)
        .await
        .map_err(|e| ImporterError::Internal(anyhow!("Failed to create dir: {:?}", e)))?;
    } else {
      if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)
          .await
          .map_err(|e| ImporterError::Internal(anyhow!("Failed to create parent dir: {:?}", e)))?;
      }

      // Create and write the file
      let mut outfile = File::create(&output_path)
        .await
        .map_err(|e| ImporterError::Internal(anyhow!("Failed to create file: {:?}", e)))?;

      let mut buffer = vec![];
      entry
        .read_to_end(&mut buffer)
        .map_err(|e| ImporterError::Internal(anyhow!("Failed to read entry content: {:?}", e)))?;

      if buffer.len() >= 4 {
        let four_bytes: [u8; 4] = buffer[..4].try_into().unwrap();
        if is_multi_part_zip_signature(&four_bytes) {
          if let Some(file_name) = Path::new(&filename).file_stem().and_then(|s| s.to_str()) {
            root_dir = Some(remove_part_suffix(file_name));
          }
          parts.push(output_path.clone());
        }
      }

      outfile
        .write_all(&buffer)
        .await
        .map_err(|e| ImporterError::Internal(anyhow!("Failed to write file: {:?}", e)))?;
    }
  }

  // Process multipart zip files
  if !parts.is_empty() {
    for part in &parts {
      let part_file = File::open(part).await?;
      let _ = unzip_single_file(part_file, &out_dir, root_dir.clone()).await?;
      fs::remove_file(part).await?;
    }
  }

  // Move all unzipped file content into parent
  match root_dir {
    None => match default_file_name {
      None => Err(ImporterError::FileNotFound),
      Some(default_file_name) => {
        let new_out_dir = out_dir
          .parent()
          .ok_or_else(|| ImporterError::FileNotFound)?
          .join(uuid::Uuid::new_v4().to_string())
          .join(&default_file_name);
        move_all(&out_dir, &new_out_dir).await?;
        fs::remove_dir_all(&out_dir).await?;
        Ok(UnzipFile {
          file_name: default_file_name,
          unzip_dir_path: new_out_dir,
          parts,
        })
      },
    },
    Some(file_name) => Ok(UnzipFile {
      file_name: file_name.clone(),
      unzip_dir_path: out_dir.join(file_name),
      parts,
    }),
  }
}

/// Helper function to move all files and directories from one path to another
#[async_recursion]
async fn move_all(old_path: &Path, new_path: &Path) -> io::Result<()> {
  if !new_path.exists() {
    fs::create_dir_all(new_path).await?;
  }

  let mut read_dir = fs::read_dir(old_path).await?;
  while let Some(entry) = read_dir.next_entry().await? {
    let path = entry.path();
    let file_name = match path.file_name() {
      Some(name) => name,
      None => continue,
    };

    let new_file_path = new_path.join(file_name);
    if path.is_dir() {
      move_all(&path, &new_file_path).await?;
      fs::remove_dir_all(&path).await?;
    } else {
      fs::rename(&path, &new_file_path).await?;
    }
  }
  Ok(())
}
