use crate::zip_tool::{is_multi_part_zip, is_multi_part_zip_file, unzip_file, unzip_stream};
use anyhow::Error;
use anyhow::Result;
use async_zip::base::read::stream::ZipFileReader;
use base64::engine::general_purpose::URL_SAFE;
use base64::Engine;
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use tokio::io::{AsyncReadExt, BufReader};

use tracing::warn;

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

pub async fn unzip_from_path_or_memory(input: Either<PathBuf, Vec<u8>>, out: PathBuf) -> PathBuf {
  match input {
    Either::Left(path) => {
      if is_multi_part_zip(&path).await.unwrap_or(false) {
        warn!(
          "This test does not support multi-part zip files: {}",
          path.display()
        );
      }
      // let file = tokio::fs::File::open(&path).await.unwrap();
      // let reader = BufReader::new(file).compat();
      // let zip_reader = ZipFileReader::new(reader);

      // let mut buffer = Vec::new();
      // file.read_to_end(&mut buffer).await.unwrap();
      // let reader = BufReader::new(&buffer[..]).compat();
      // let zip_reader = ZipFileReader::new(reader);
      // unzip_async(zip_reader, out).await.unwrap()

      let file = tokio::fs::File::open(&path).await.unwrap();
      unzip_file(file, &out, None).await.unwrap().unzip_dir_path
    },
    Either::Right(data) => {
      if data.len() >= 4 {
        if let Ok(first_4_bytes) = data[..4].try_into() {
          if is_multi_part_zip_file(first_4_bytes) {
            warn!("This test does not support multi-part zip files");
          }
        }
      }

      let zip_reader = ZipFileReader::new(data.as_slice());
      unzip_stream(zip_reader, out, None)
        .await
        .unwrap()
        .unzip_dir_path
    },
  }
}

pub enum Either<L, R> {
  Left(L),
  Right(R),
}
