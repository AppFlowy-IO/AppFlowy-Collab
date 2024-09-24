use crate::error::DatabaseError;
use collab::entity::EncodedCollab;
use collab::preclude::Collab;
use collab_entity::CollabType;

use anyhow::Error;
use base64::engine::general_purpose::URL_SAFE;
use base64::Engine;
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::PathBuf;

pub(crate) fn encoded_collab(
  collab: &Collab,
  collab_type: &CollabType,
) -> Result<EncodedCollab, DatabaseError> {
  let encoded_collab =
    collab.encode_collab_v1(|collab| collab_type.validate_require_data(collab))?;
  Ok(encoded_collab)
}

pub fn upload_file_url(host: &str, workspace_id: &str, object_id: &str, file_id: &str) -> String {
  let parent_dir = utf8_percent_encode(object_id, NON_ALPHANUMERIC).to_string();
  format!("{host}/{workspace_id}/v1/blob/{parent_dir}/{file_id}",)
}

pub struct DatabaseFileId;

impl DatabaseFileId {
  pub fn from_path(file_path: &PathBuf) -> Result<String, Error> {
    sync_calculate_file_id(file_path)
  }

  pub fn from_bytes(bytes: &[u8], ext: String) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let hash_result = hasher.finalize();
    format!("{}.{}", URL_SAFE.encode(hash_result), ext)
  }
}

pub fn sync_calculate_file_id(file_path: &PathBuf) -> Result<String, Error> {
  let file = std::fs::File::open(file_path)?;
  let ext = file_path
    .extension()
    .and_then(std::ffi::OsStr::to_str)
    .unwrap_or("")
    .to_owned();

  let mut reader = std::io::BufReader::new(file);
  let mut buffer = vec![0u8; 1024 * 1024]; // 1MB buffer size
  let mut hasher = Sha256::new();

  while let Ok(bytes_read) = reader.read(&mut buffer) {
    if bytes_read == 0 {
      break;
    }
    hasher.update(&buffer[..bytes_read]);
  }

  let hash_result = hasher.finalize();
  let file_id = format!("{}.{}", URL_SAFE.encode(hash_result), ext);
  Ok(file_id)
}
