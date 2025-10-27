use crate::error::CollabError;
use crate::importer::zip_tool::util::{
  has_multi_part_extension, has_multi_part_suffix, is_multi_part_zip_signature, remove_part_suffix,
  sanitize_file_path,
};
use anyhow::{Result, anyhow};

use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::{fs, io};
use tracing::{trace, warn};
use zip::read::ZipArchive;

pub struct UnzipFile {
  pub dir_name: String,
  pub unzip_dir: PathBuf,
  pub parts: Vec<PathBuf>,
}

pub fn sync_unzip(
  file_path: PathBuf,
  out_dir: PathBuf,
  default_file_name: Option<String>,
) -> Result<UnzipFile, CollabError> {
  let file = File::open(file_path)
    .map_err(|e| CollabError::Internal(anyhow!("Failed to open zip file: {:?}", e)))?;

  let mut archive = ZipArchive::new(file)
    .map_err(|e| CollabError::Internal(anyhow!("Failed to read zip archive: {:?}", e)))?;

  let mut root_dir = None;
  let mut parts = vec![];

  // Determine the root directory if the first entry is a directory
  if let Ok(entry) = archive.by_index(0) {
    let filename = entry.name().to_string();
    if root_dir.is_none() && entry.is_dir() {
      root_dir = Some(filename.split('/').next().unwrap_or(&filename).to_string());
    }
  }

  if !out_dir.exists() {
    fs::create_dir_all(&out_dir)
      .map_err(|e| CollabError::Internal(anyhow!("Failed to create dir: {:?}", e)))?;
  }

  // Iterate through each file in the archive
  for i in 0..archive.len() {
    let mut entry = archive
      .by_index(i)
      .map_err(|e| CollabError::Internal(anyhow!("Failed to read entry: {:?}", e)))?;

    let filename = entry.name().to_string();
    // Skip zip files within subdirectories
    if entry.is_file() && filename.ends_with(".zip") && i != 0 {
      trace!("Skipping zip file: {:?}", filename);
      continue;
    }

    let output_path = out_dir.join(&filename);
    if entry.is_dir() {
      fs::create_dir_all(&output_path)
        .map_err(|e| CollabError::Internal(anyhow!("Failed to create dir: {:?}", e)))?;
    } else {
      if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)
          .map_err(|e| CollabError::Internal(anyhow!("Failed to create parent dir: {:?}", e)))?;
      }

      // Create and write the file
      if output_path.exists() {
        trace!(
          "File {:?} already exists; overwriting extracted content",
          output_path
        );
      }

      match OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&output_path)
        .map_err(|e| {
          CollabError::Internal(anyhow!(
            "Failed to create or overwrite file with path: {:?}, error: {:?}",
            output_path,
            e
          ))
        }) {
        Ok(mut outfile) => {
          let mut buffer = vec![];
          entry
            .read_to_end(&mut buffer)
            .map_err(|e| CollabError::Internal(anyhow!("Failed to read entry content: {:?}", e)))?;

          // Check if it's a multipart zip file
          if buffer.len() >= 4 {
            let four_bytes: [u8; 4] = buffer[..4].try_into().unwrap();
            if is_multi_part_zip_signature(&four_bytes) {
              let is_multipart_candidate = filename.contains('/')
                || has_multi_part_extension(&filename)
                || has_multi_part_suffix(&filename);

              if root_dir.is_none() && is_multipart_candidate {
                if let Some(file_name) = Path::new(&filename).file_stem().and_then(|s| s.to_str()) {
                  root_dir = Some(remove_part_suffix(file_name));
                }
              }
              if is_multipart_candidate {
                parts.push(output_path.clone());
              }
            }
          }

          outfile
            .write_all(&buffer)
            .map_err(|e| CollabError::Internal(anyhow!("Failed to write file: {:?}", e)))?;
        },
        Err(err) => {
          warn!("{}", err);
        },
      }
    }
  }
  drop(archive);

  // Process multipart zip files
  if !parts.is_empty() {
    for part in &parts {
      let part_file = fs::File::open(part)?;
      let _ = unzip_single_file(part_file, &out_dir, root_dir.clone())?;
      fs::remove_file(part)?;
    }
  }

  // Move all unzipped file content into parent
  match root_dir {
    None => match default_file_name {
      None => Err(CollabError::ImporterFileNotFound),
      Some(root_dir) => Ok(UnzipFile {
        dir_name: root_dir,
        unzip_dir: out_dir,
        parts,
      }),
    },
    Some(root_dir) => {
      let target_dir = out_dir.join(&root_dir);
      if !target_dir.exists() {
        warn!(
          "Root directory {:?} missing after unzip; falling back to {:?}",
          target_dir, out_dir
        );
        return Ok(UnzipFile {
          dir_name: root_dir,
          unzip_dir: out_dir,
          parts,
        });
      }

      Ok(UnzipFile {
        dir_name: root_dir.clone(),
        unzip_dir: target_dir,
        parts,
      })
    },
  }
}

fn unzip_single_file(
  archive_file: File,
  out_dir: &Path,
  mut root_dir: Option<String>,
) -> Result<UnzipFile, CollabError> {
  let mut archive = ZipArchive::new(archive_file)
    .map_err(|e| CollabError::Internal(anyhow!("Failed to read zip archive: {:?}", e)))?;

  // Iterate through each file in the archive
  for i in 0..archive.len() {
    let mut entry = archive
      .by_index(i)
      .map_err(|e| CollabError::Internal(anyhow!("Failed to read entry: {:?}", e)))?;

    let entry_name = entry.name();
    if entry_name == ".DS_Store" || entry_name.starts_with("__MACOSX") {
      continue;
    }

    let file_name = entry.name().to_string();
    if root_dir.is_none() && entry.is_dir() {
      root_dir = Some(
        file_name
          .split('/')
          .next()
          .unwrap_or(&file_name)
          .to_string(),
      );
    }

    let path = out_dir.join(sanitize_file_path(&file_name));
    // Create directories if needed
    if entry.is_dir() {
      if !path.exists() {
        fs::create_dir_all(&path)
          .map_err(|e| CollabError::Internal(anyhow!("Failed to create directory: {:?}", e)))?;
      }
    } else {
      // Ensure parent directories exist
      if let Some(parent) = path.parent() {
        if !parent.exists() {
          fs::create_dir_all(parent).map_err(|e| {
            CollabError::Internal(anyhow!("Failed to create parent directory: {:?}", e))
          })?;
        }
      }

      // Create and write the file
      if path.exists() {
        trace!(
          "File {:?} already exists when extracting multipart entry; overwriting",
          path
        );
      }

      let mut outfile = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&path)
        .map_err(|e| {
          CollabError::Internal(anyhow!(
            "Failed to create or overwrite part file: {:?}, path:{:?}",
            e,
            path
          ))
        })?;

      io::copy(&mut entry, &mut outfile)
        .map_err(|e| CollabError::Internal(anyhow!("Failed to write file: {:?}", e)))?;
    }
  }

  // Return result with root directory info
  match root_dir {
    None => Err(CollabError::ImporterFileNotFound),
    Some(root_dir) => Ok(UnzipFile {
      dir_name: root_dir.clone(),
      unzip_dir: out_dir.join(root_dir),
      parts: vec![],
    }),
  }
}

// this function will not return parts
pub fn sync_simple_unzip(
  zip_path: PathBuf,
  output_dir: PathBuf,
  workspace_name: Option<String>,
) -> Result<UnzipFile, CollabError> {
  let file = File::open(&zip_path)
    .map_err(|e| CollabError::Internal(anyhow!("Failed to open zip file: {:?}", e)))?;

  let mut archive = ZipArchive::new(file)
    .map_err(|e| CollabError::Internal(anyhow!("Failed to read zip archive: {:?}", e)))?;

  let output_dir = if let Some(name) = workspace_name {
    output_dir.join(name)
  } else {
    output_dir.join(format!("workspace_{}", uuid::Uuid::new_v4()))
  };

  if !output_dir.exists() {
    std::fs::create_dir_all(&output_dir)
      .map_err(|e| CollabError::Internal(anyhow!("Failed to create output directory: {:?}", e)))?;
  }

  for i in 0..archive.len() {
    let mut file = archive
      .by_index(i)
      .map_err(|e| CollabError::Internal(anyhow!("Failed to read entry: {:?}", e)))?;

    let output_path = match file.enclosed_name() {
      Some(path) => output_dir.join(path),
      None => continue,
    };

    if file.is_dir() {
      std::fs::create_dir_all(&output_path)
        .map_err(|e| CollabError::Internal(anyhow!("Failed to create directory: {:?}", e)))?;
    } else {
      if let Some(p) = output_path.parent() {
        if !p.exists() {
          std::fs::create_dir_all(p).map_err(|e| {
            CollabError::Internal(anyhow!("Failed to create parent directory: {:?}", e))
          })?;
        }
      }

      let mut outfile = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&output_path)
        .map_err(|e| CollabError::Internal(anyhow!("Failed to create file: {:?}", e)))?;

      std::io::copy(&mut file, &mut outfile)
        .map_err(|e| CollabError::Internal(anyhow!("Failed to extract file: {:?}", e)))?;
    }
  }

  Ok(UnzipFile {
    dir_name: output_dir.to_string_lossy().to_string(),
    unzip_dir: output_dir,
    parts: vec![],
  })
}
