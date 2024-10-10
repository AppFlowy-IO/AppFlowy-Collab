use anyhow::Error;
use anyhow::{Context, Result};
use base64::engine::general_purpose::URL_SAFE;
use base64::Engine;
use sha2::{Digest, Sha256};
use std::io::Read;
use std::io::{Cursor, Seek};
use std::path::PathBuf;
use tokio::fs;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tokio::io::{AsyncReadExt, BufReader};
use zip::ZipArchive;

pub fn upload_file_url(host: &str, workspace_id: &str, object_id: &str, file_id: &str) -> String {
  format!("{host}/api/file_storage/{workspace_id}/v1/blob/{object_id}/{file_id}",)
}

pub struct FileId;

impl FileId {
  pub async fn from_path(file_path: &PathBuf) -> Result<String, Error> {
    async_calculate_file_id(file_path).await
  }

  pub fn from_bytes(bytes: &[u8], ext: String) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let hash_result = hasher.finalize();
    format!("{}.{}", URL_SAFE.encode(hash_result), ext)
  }
}

async fn async_calculate_file_id(file_path: &PathBuf) -> Result<String, Error> {
  let file = tokio::fs::File::open(file_path).await?;
  let ext = file_path
    .extension()
    .and_then(std::ffi::OsStr::to_str)
    .unwrap_or("")
    .to_owned();

  let mut reader = BufReader::new(file);
  let mut buffer = vec![0u8; 1024 * 1024];
  let mut hasher = Sha256::new();
  while let Ok(bytes_read) = reader.read(&mut buffer).await {
    if bytes_read == 0 {
      break;
    }
    hasher.update(&buffer[..bytes_read]);
  }
  let hash_result = hasher.finalize();
  let file_id = format!("{}.{}", URL_SAFE.encode(hash_result), ext);
  Ok(file_id)
}

pub async fn unzip<R: Read + Seek>(
  mut archive: ZipArchive<R>,
  file_name: &str,
  out: PathBuf,
) -> Result<PathBuf> {
  for i in 0..archive.len() {
    let mut file = archive.by_index(i)?;
    let outpath = out.join(file.mangled_name());
    if file.name().ends_with('/') {
      fs::create_dir_all(&outpath).await?;
    } else {
      if let Some(parent) = outpath.parent() {
        if !parent.exists() {
          fs::create_dir_all(parent).await?;
        }
      }

      let mut outfile = File::create(&outpath)
        .await
        .with_context(|| format!("Failed to create file: {:?}", outpath))?;

      let mut buffer = Vec::new();
      file.read_to_end(&mut buffer).with_context(|| {
        format!(
          "Failed to read contents of file in archive: {}",
          file.name()
        )
      })?;

      outfile
        .write_all(&buffer)
        .await
        .with_context(|| format!("Failed to write file contents to: {:?}", outpath))?;
    }
  }
  Ok(out.join(file_name))
}

pub async fn unzip_from_path_or_memory(
  input: Either<PathBuf, (Vec<u8>, String)>,
  out: PathBuf,
) -> Result<PathBuf> {
  match input {
    Either::Left(path) => {
      let file_name = path
        .file_stem()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "Invalid file stem"))?
        .to_str()
        .ok_or_else(|| {
          std::io::Error::new(std::io::ErrorKind::InvalidInput, "Invalid file name")
        })?;

      let reader = std::fs::File::open(&path)
        .with_context(|| format!("Failed to open file at path: {:?}", path))?;
      let archive = ZipArchive::new(reader)?;
      unzip(archive, file_name, out).await
    },
    Either::Right((data, file_name)) => {
      let archive = ZipArchive::new(Cursor::new(data))?;
      unzip(archive, &file_name, out).await
    },
  }
}

pub enum Either<L, R> {
  Left(L),
  Right(R),
}
