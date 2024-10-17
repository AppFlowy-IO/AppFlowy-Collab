use anyhow::{anyhow, Context, Result};
use async_recursion::async_recursion;
use async_zip::base::read::stream::{Ready, ZipFileReader};
use async_zip::{StringEncoding, ZipString};
use futures::io::AsyncBufRead;
use futures::AsyncReadExt as FuturesAsyncReadExt;
use std::ffi::OsString;
use std::{io, str};

use std::path::{Path, PathBuf};
use tokio::fs;
use tokio::fs::File;
use tokio::io::AsyncReadExt;
use tokio::io::{AsyncWriteExt, BufReader};

use async_zip::base::read::seek::ZipFileReader as SeekZipFileReader;
use fancy_regex::Regex;
use tokio::fs::{create_dir_all, OpenOptions};
use tokio_util::compat::TokioAsyncReadCompatExt;
use tokio_util::compat::TokioAsyncWriteCompatExt;

use crate::error::ImporterError;

use tracing::error;

pub struct UnzipFile {
  pub file_name: String,
  pub unzip_dir_path: PathBuf,
  pub parts: Vec<PathBuf>,
}

#[async_recursion(?Send)]
pub async fn unzip_stream<R: AsyncBufRead + Unpin>(
  mut zip_reader: ZipFileReader<Ready<R>>,
  out_dir: PathBuf,
  default_file_name: Option<String>,
) -> Result<UnzipFile, ImporterError> {
  let mut root_dir = None;
  let mut parts = vec![];
  #[allow(irrefutable_let_patterns)]
  while let result = zip_reader.next_with_entry().await {
    match result {
      Ok(Some(mut next_reader)) => {
        let entry_reader = next_reader.reader_mut();
        let filename = get_filename_from_zip_string(entry_reader.entry().filename())
          .with_context(|| "Failed to extract filename from entry".to_string())?;

        if root_dir.is_none() && entry_reader.entry().dir().unwrap_or(false) {
          root_dir = Some(filename.split('/').next().unwrap_or(&filename).to_string());
        }

        let output_path = out_dir.join(&filename);
        if filename.ends_with('/') {
          fs::create_dir_all(&output_path)
            .await
            .with_context(|| format!("Failed to create directory: {}", output_path.display()))?;
        } else {
          // Ensure parent directories exist
          if let Some(parent) = output_path.parent() {
            if !parent.exists() {
              fs::create_dir_all(parent).await.with_context(|| {
                format!("Failed to create parent directory: {}", parent.display())
              })?;
            }
          }

          // Write file contents
          if let Ok(mut outfile) = File::create(&output_path).await {
            let mut buffer = vec![];
            match entry_reader.read_to_end(&mut buffer).await {
              Ok(_) => {
                if buffer.len() >= 4 {
                  if let Ok(four_bytes) = buffer[..4].try_into() {
                    if is_multi_part_zip_signature(four_bytes) {
                      if let Some(file_name) =
                        Path::new(&filename).file_stem().and_then(|s| s.to_str())
                      {
                        root_dir = Some(remove_part_suffix(file_name));
                      }
                      parts.push(output_path.clone());
                    }
                  }
                }

                outfile.write_all(&buffer).await.with_context(|| {
                  format!("Failed to write data to file: {}", output_path.display())
                })?;
              },
              Err(err) => {
                error!(
                  "Failed to read entry: {:?}. Error: {:?}",
                  entry_reader.entry(),
                  err,
                );
                return Err(ImporterError::Internal(anyhow!(
                  "Unexpected EOF while reading: {}",
                  filename
                )));
              },
            }
          }
        }

        // Move to the next file in the zip
        zip_reader = next_reader
          .done()
          .await
          .with_context(|| "Failed to move to the next entry")?;
      },
      Ok(None) => break,
      Err(zip_error) => {
        error!("Error reading zip file: {:?}", zip_error);
        break;
      },
    }
  }

  if !parts.is_empty() {
    for part in &parts {
      let part_file = File::open(part).await?;
      let _ = unzip_file(part_file, &out_dir, root_dir.clone()).await?;
      let _ = fs::remove_file(part).await;
    }
  }

  // move all unzip file content into parent
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
        let _ = fs::remove_dir_all(&out_dir).await;
        Ok(UnzipFile {
          file_name: default_file_name.clone(),
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
      if !new_file_path.exists() {
        fs::create_dir_all(&new_file_path).await?;
      }
      move_all(&path, &new_file_path).await?;
      fs::remove_dir_all(&path).await?;
    } else if path.is_file() {
      fs::rename(&path, &new_file_path).await?;
    }
  }
  Ok(())
}

pub fn get_filename_from_zip_string(zip_string: &ZipString) -> Result<String, anyhow::Error> {
  match zip_string.encoding() {
    StringEncoding::Utf8 => match zip_string.as_str() {
      Ok(valid_str) => Ok(valid_str.to_string()),
      Err(err) => Err(err.into()),
    },

    StringEncoding::Raw => {
      let raw_bytes = zip_string.as_bytes();
      let utf8_str = str::from_utf8(raw_bytes)?;
      let os_string = OsString::from(utf8_str);
      Ok(os_string.to_string_lossy().into_owned())
    },
  }
}

/// Check if the first 4 bytes of the buffer match known multi-part zip signatures.
fn is_multi_part_zip_signature(buffer: &[u8; 4]) -> bool {
  const MULTI_PART_SIGNATURES: [[u8; 4]; 2] = [
    [0x50, 0x4b, 0x07, 0x08], // Spanned zip signature
    [0x50, 0x4b, 0x03, 0x04], // Regular zip signature
  ];

  MULTI_PART_SIGNATURES.contains(buffer)
}

/// Async function to check if a file is a multi-part zip by reading the first 4 bytes.
pub async fn is_multi_part_zip(file_path: &Path) -> Result<bool> {
  let mut file = File::open(file_path).await?;
  let mut buffer = [0; 4]; // Read only the first 4 bytes
  file.read_exact(&mut buffer).await?;
  Ok(is_multi_part_zip_signature(&buffer))
}

/// Check if a buffer contains the multi-part zip signature.
pub fn is_multi_part_zip_file(buffer: &[u8; 4]) -> bool {
  is_multi_part_zip_signature(buffer)
}

fn sanitize_file_path(path: &str) -> PathBuf {
  // Replaces backwards slashes
  path.replace('\\', "/")
      // Sanitizes each component
      .split('/')
      .map(sanitize_filename::sanitize)
      .collect()
}

/// Extracts everything from the ZIP archive to the output directory
pub async fn unzip_file(
  archive: File,
  out_dir: &Path,
  mut root_dir: Option<String>,
) -> Result<UnzipFile, ImporterError> {
  let archive = BufReader::new(archive).compat();
  let mut reader = SeekZipFileReader::new(archive)
    .await
    .map_err(|err| ImporterError::Internal(err.into()))?;

  for index in 0..reader.file().entries().len() {
    let entry = reader.file().entries().get(index).unwrap();
    let file_name = entry
      .filename()
      .as_str()
      .map_err(|err| ImporterError::Internal(err.into()))?;
    if root_dir.is_none() && file_name.ends_with('/') {
      root_dir = Some(file_name.split('/').next().unwrap_or(file_name).to_string());
    }

    let path = out_dir.join(sanitize_file_path(file_name));
    // If the filename of the entry ends with '/', it is treated as a directory.
    // This is implemented by previous versions of this crate and the Python Standard Library.
    // https://docs.rs/async_zip/0.0.8/src/async_zip/read/mod.rs.html#63-65
    // https://github.com/python/cpython/blob/820ef62833bd2d84a141adedd9a05998595d6b6d/Lib/zipfile.py#L528
    let entry_is_dir = entry
      .dir()
      .map_err(|err| ImporterError::Internal(err.into()))?;
    let mut entry_reader = reader
      .reader_without_entry(index)
      .await
      .map_err(|err| ImporterError::Internal(err.into()))?;

    if entry_is_dir {
      if !path.exists() {
        create_dir_all(&path).await?;
      }
    } else {
      // Creates parent directories. They may not exist if iteration is out of order
      // or the archive does not contain directory entries.
      if let Some(parent) = path.parent() {
        if !parent.is_dir() {
          create_dir_all(parent).await?;
        }
      }
      let writer = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .await?;
      futures_lite::io::copy(&mut entry_reader, &mut writer.compat_write()).await?;
    }
  }
  match root_dir {
    None => Err(ImporterError::FileNotFound),
    Some(root_dir) => Ok(UnzipFile {
      file_name: root_dir.clone(),
      unzip_dir_path: out_dir.join(root_dir),
      parts: vec![],
    }),
  }
}

fn remove_part_suffix(file_name: &str) -> String {
  let path = Path::new(file_name);
  if let Some(stem) = path.file_stem() {
    let mut stem_str = stem.to_string_lossy().to_string();
    // Common patterns for multi-part files
    // Common patterns for multi-part files
    let patterns = [
      r"(?i)-part-\d+", // -Part-1, -Part-2, etc., case-insensitive
      r"(?i)\.z\d{2}",  // .z01, .z02, etc., case-insensitive
      r"(?i)\.part\d+", // .part1, .part2, etc., case-insensitive
      r"\(\d+\)",       // (1), (2), etc.
      r"_\d+",          // _1, _2, etc.
    ];

    for pattern in &patterns {
      let re = Regex::new(pattern).unwrap();
      stem_str = re.replace(&stem_str, "").to_string();
    }
    return stem_str;
  }

  file_name.to_string()
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_remove_part_suffix() {
    let cases = vec![
      // Test cases with expected outputs
      (
        "Export-99d4faad-5bc1-4fef-82ac-5f6c7a957b12-Part-1.zip",
        "Export-99d4faad-5bc1-4fef-82ac-5f6c7a957b12",
      ),
      (
        "Export-99d4faad-5bc1-4fef-82ac-5f6c7a957b12-part-1.zip",
        "Export-99d4faad-5bc1-4fef-82ac-5f6c7a957b12",
      ),
      ("file.z01", "file"),
      ("file.part2.zip", "file"),
      ("file(1).zip", "file"),
      ("file_3.zip", "file"),
      ("document-Part-10.zip", "document"),
      ("project.part1.zip", "project"),
      ("archive.z99.zip", "archive"),
      // Test case with no suffix
      ("normalfile.zip", "normalfile"),
      // Test case with no extension
      ("file-no-ext", "file-no-ext"),
    ];

    for (input, expected) in cases {
      assert_eq!(
        remove_part_suffix(input),
        expected,
        "Failed for input: {}",
        input
      );
    }
  }
}
