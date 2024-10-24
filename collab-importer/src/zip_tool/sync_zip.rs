use crate::error::ImporterError;
use crate::zip_tool::util::{is_multi_part_zip_signature, remove_part_suffix, sanitize_file_path};
use anyhow::{anyhow, Result};

use std::fs::{File, OpenOptions, Permissions};
use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::{fs, io};
use tracing::warn;
use zip::read::ZipArchive;

pub struct UnzipFile {
  pub dir_name: String,
  pub unzip_dir: PathBuf,
  pub parts: Vec<PathBuf>,
}

pub fn sync_unzip(
  file_path: PathBuf,
  mut out_dir: PathBuf,
  default_file_name: Option<String>,
) -> Result<UnzipFile, ImporterError> {
  let file = File::open(file_path)
    .map_err(|e| ImporterError::Internal(anyhow!("Failed to open zip file: {:?}", e)))?;

  let mut archive = ZipArchive::new(file)
    .map_err(|e| ImporterError::Internal(anyhow!("Failed to read zip archive: {:?}", e)))?;

  let mut root_dir = None;
  let mut parts = vec![];

  if let Ok(entry) = archive.by_index(0) {
    let filename = entry.name().to_string();
    if root_dir.is_none() && entry.is_dir() {
      root_dir = Some(filename.split('/').next().unwrap_or(&filename).to_string());
    }
  }

  if root_dir.is_none() {
    if let Some(default_name) = &default_file_name {
      out_dir = out_dir.join(default_name);
      if !out_dir.exists() {
        fs::create_dir_all(&out_dir)
          .map_err(|e| ImporterError::Internal(anyhow!("Failed to create dir: {:?}", e)))?;
      }
    }
  }

  // Iterate through each file in the archive
  for i in 0..archive.len() {
    let mut entry = archive
      .by_index(i)
      .map_err(|e| ImporterError::Internal(anyhow!("Failed to read entry: {:?}", e)))?;

    let filename = entry.name().to_string();
    let output_path = out_dir.join(&filename);
    if entry.is_dir() {
      fs::create_dir_all(&output_path)
        .map_err(|e| ImporterError::Internal(anyhow!("Failed to create dir: {:?}", e)))?;
    } else {
      if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)
          .map_err(|e| ImporterError::Internal(anyhow!("Failed to create parent dir: {:?}", e)))?;
      }

      // Create and write the file
      match OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&output_path)
        .map_err(|e| {
          ImporterError::Internal(anyhow!(
            "Failed to create or open file with path: {:?}, error:{:?}",
            output_path,
            e
          ))
        }) {
        Ok(mut outfile) => {
          let mut buffer = vec![];
          entry.read_to_end(&mut buffer).map_err(|e| {
            ImporterError::Internal(anyhow!("Failed to read entry content: {:?}", e))
          })?;

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
            .map_err(|e| ImporterError::Internal(anyhow!("Failed to write file: {:?}", e)))?;
        },
        Err(err) => {
          warn!("{}", err);
        },
      }
    }
  }

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
      None => Err(ImporterError::FileNotFound),
      Some(root_dir) => Ok(UnzipFile {
        dir_name: root_dir,
        unzip_dir: out_dir,
        parts,
      }),
    },
    Some(root_dir) => Ok(UnzipFile {
      dir_name: root_dir.clone(),
      unzip_dir: out_dir.join(root_dir),
      parts,
    }),
  }
}

/// Helper function to move all files and directories from one path to another
#[allow(dead_code)]
fn move_all(old_path: &Path, new_path: &Path) -> Result<(), ImporterError> {
  if !new_path.exists() {
    fs::create_dir_all(new_path)?;
    let dir_permissions = Permissions::from_mode(0o755);
    fs::set_permissions(new_path, dir_permissions)?;
  }

  for entry in fs::read_dir(old_path).map_err(|err| {
    ImporterError::Internal(anyhow!("Can not read {:?}, error: {:?}", old_path, err))
  })? {
    let entry = entry?;
    let path = entry.path();
    let file_name = match path.file_name() {
      Some(name) => name,
      None => continue,
    };

    let new_file_path = new_path.join(file_name);

    if path.is_dir() {
      move_all(&path, &new_file_path)?;
      fs::remove_dir_all(&path).map_err(|err| {
        ImporterError::Internal(anyhow!(
          "Can not remove directory {:?}, error: {:?}",
          path,
          err
        ))
      })?;
    } else {
      fs::rename(&path, &new_file_path).map_err(|err| {
        ImporterError::Internal(anyhow!(
          "Can not move file {:?} to {:?}, error: {:?}",
          path,
          new_file_path,
          err
        ))
      })?;
    }
  }
  Ok(())
}
fn unzip_single_file(
  archive_file: File,
  out_dir: &Path,
  mut root_dir: Option<String>,
) -> Result<UnzipFile, ImporterError> {
  let mut archive = ZipArchive::new(archive_file)
    .map_err(|e| ImporterError::Internal(anyhow!("Failed to read zip archive: {:?}", e)))?;

  // Iterate through each file in the archive
  for i in 0..archive.len() {
    let mut entry = archive
      .by_index(i)
      .map_err(|e| ImporterError::Internal(anyhow!("Failed to read entry: {:?}", e)))?;

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
          .map_err(|e| ImporterError::Internal(anyhow!("Failed to create directory: {:?}", e)))?;
      }
    } else {
      // Ensure parent directories exist
      if let Some(parent) = path.parent() {
        if !parent.exists() {
          fs::create_dir_all(parent).map_err(|e| {
            ImporterError::Internal(anyhow!("Failed to create parent directory: {:?}", e))
          })?;
        }
      }

      // Create and write the file
      let mut outfile = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|e| ImporterError::Internal(anyhow!("Failed to create part file: {:?}", e)))?;

      io::copy(&mut entry, &mut outfile)
        .map_err(|e| ImporterError::Internal(anyhow!("Failed to write file: {:?}", e)))?;
    }
  }

  // Return result with root directory info
  match root_dir {
    None => Err(ImporterError::FileNotFound),
    Some(root_dir) => Ok(UnzipFile {
      dir_name: root_dir.clone(),
      unzip_dir: out_dir.join(root_dir),
      parts: vec![],
    }),
  }
}
