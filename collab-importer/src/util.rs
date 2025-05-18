use crate::zip_tool::async_zip::{async_unzip, unzip_single_file};
use anyhow::Error;
use anyhow::Result;
use async_zip::base::read::stream::ZipFileReader;
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE;
use percent_encoding::percent_decode_str;
use sha2::{Digest, Sha256};
use std::ops::Deref;
use std::path::PathBuf;
use tokio::io::{AsyncReadExt, BufReader};

use crate::error::ImporterError;
use crate::zip_tool::util::{is_multi_part_zip, is_multi_part_zip_file};
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

pub async fn unzip_from_path_or_memory(
  input: Either<PathBuf, Vec<u8>>,
  out: PathBuf,
) -> Result<PathBuf, ImporterError> {
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
      Ok(unzip_single_file(file, &out, None).await?.unzip_dir_path)
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
      Ok(async_unzip(zip_reader, out, None).await?.unzip_dir_path)
    },
  }
}

pub enum Either<L, R> {
  Left(L),
  Right(R),
}

pub fn parse_csv(file_path: &PathBuf) -> CSVFile {
  let content = std::fs::read_to_string(file_path).unwrap();
  let mut reader = csv::Reader::from_reader(content.as_bytes());
  let csv_fields = reader
    .headers()
    .unwrap()
    .iter()
    .map(|s| s.to_string())
    .collect::<Vec<String>>();
  let csv_rows = reader
    .records()
    .flat_map(|r| r.ok())
    .map(|record| {
      record
        .into_iter()
        .filter_map(|s| Some(percent_decode_str(s).decode_utf8().ok()?.to_string()))
        .collect::<Vec<String>>()
    })
    .collect::<Vec<Vec<String>>>();

  CSVFile {
    columns: csv_fields,
    rows: csv_rows.into_iter().map(|cells| CSVRow { cells }).collect(),
  }
}

#[derive(Debug, Clone)]
pub struct CSVFile {
  pub columns: Vec<String>,
  pub rows: Vec<CSVRow>,
}

#[derive(Debug, Clone)]
pub struct CSVRow {
  cells: Vec<String>,
}
impl Deref for CSVRow {
  type Target = Vec<String>;

  fn deref(&self) -> &Self::Target {
    &self.cells
  }
}
