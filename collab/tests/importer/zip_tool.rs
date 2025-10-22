use anyhow::Result;
use std::io::{Cursor, Write};
use std::path::Path;

use collab::importer::zip_tool::async_zip::async_unzip;
use collab::importer::zip_tool::sync_zip::sync_unzip;
use tempfile::tempdir;
use tokio_util::compat::TokioAsyncReadCompatExt;
use zip::CompressionMethod;
use zip::ZipWriter;
use zip::write::FileOptions;

const ROOT_DIR: &str = "ExportBlock-7365b89b";
const FALLBACK_DIR: &str = "fallback";

#[test]
fn sync_unzip_preserves_root_directory_with_nested_zip() -> Result<()> {
  let temp = tempdir()?;
  let zip_path = temp.path().join("root_dir_with_nested.zip");
  create_zip_with_root_dir(&zip_path)?;

  let output_dir = temp.path().join("output");
  std::fs::create_dir_all(&output_dir)?;

  let unzip_file = sync_unzip(
    zip_path.clone(),
    output_dir.clone(),
    Some(FALLBACK_DIR.to_string()),
  )?;

  let expected_root = output_dir.join(ROOT_DIR);
  assert_eq!(unzip_file.dir_name, ROOT_DIR);
  assert_eq!(unzip_file.unzip_dir, expected_root);
  assert!(unzip_file.unzip_dir.is_dir());

  Ok(())
}

#[test]
fn sync_unzip_falls_back_when_root_directory_missing() -> Result<()> {
  let temp = tempdir()?;
  let zip_path = temp.path().join("missing_root.zip");
  create_zip_without_root_dir(&zip_path)?;

  let output_dir = temp.path().join("output");
  std::fs::create_dir_all(&output_dir)?;

  let unzip_file = sync_unzip(
    zip_path.clone(),
    output_dir.clone(),
    Some(FALLBACK_DIR.to_string()),
  )?;

  assert_eq!(unzip_file.dir_name, FALLBACK_DIR);
  assert_eq!(unzip_file.unzip_dir, output_dir);
  assert!(unzip_file.unzip_dir.is_dir());

  Ok(())
}

#[tokio::test]
async fn async_unzip_preserves_root_directory_with_nested_zip() -> Result<()> {
  let temp = tempdir()?;
  let zip_path = temp.path().join("async_root_dir_with_nested.zip");
  create_zip_with_root_dir(&zip_path)?;

  let file = tokio::fs::File::open(&zip_path).await?;
  let reader = tokio::io::BufReader::new(file).compat();
  let zip_reader = async_zip::base::read::stream::ZipFileReader::new(reader);

  let output_dir = temp.path().join("async_output");
  tokio::fs::create_dir_all(&output_dir).await?;

  let unzip_file = async_unzip(
    zip_reader,
    output_dir.clone(),
    Some(FALLBACK_DIR.to_string()),
  )
  .await?;

  let expected_root = output_dir.join(ROOT_DIR);
  assert_eq!(unzip_file.file_name, ROOT_DIR);
  assert_eq!(unzip_file.unzip_dir_path, expected_root);
  assert!(
    tokio::fs::metadata(unzip_file.unzip_dir_path.clone())
      .await?
      .is_dir()
  );

  Ok(())
}

#[tokio::test]
async fn async_unzip_falls_back_when_root_directory_missing() -> Result<()> {
  let temp = tempdir()?;
  let zip_path = temp.path().join("async_missing_root.zip");
  create_zip_without_root_dir(&zip_path)?;

  let file = tokio::fs::File::open(&zip_path).await?;
  let reader = tokio::io::BufReader::new(file).compat();
  let zip_reader = async_zip::base::read::stream::ZipFileReader::new(reader);

  let output_dir = temp.path().join("async_output");
  tokio::fs::create_dir_all(&output_dir).await?;

  let unzip_file = async_unzip(
    zip_reader,
    output_dir.clone(),
    Some(FALLBACK_DIR.to_string()),
  )
  .await?;

  assert_eq!(unzip_file.file_name, FALLBACK_DIR);
  assert_eq!(unzip_file.unzip_dir_path, output_dir);
  assert!(
    tokio::fs::metadata(&unzip_file.unzip_dir_path)
      .await?
      .is_dir()
  );

  Ok(())
}

fn create_zip_with_root_dir(zip_path: &Path) -> Result<()> {
  let file = std::fs::File::create(zip_path)?;
  let mut writer = ZipWriter::new(file);
  let options = FileOptions::default().compression_method(CompressionMethod::Stored);

  writer.add_directory(format!("{ROOT_DIR}/"), options)?;
  writer.start_file(format!("{ROOT_DIR}/{ROOT_DIR}.md"), options)?;
  writer.write_all(b"page content")?;

  writer.start_file(format!("{ROOT_DIR}/Attachment.zip"), options)?;
  writer.write_all(&create_nested_zip_bytes("nested.txt"))?;

  writer.finish()?;
  Ok(())
}

fn create_zip_without_root_dir(zip_path: &Path) -> Result<()> {
  let file = std::fs::File::create(zip_path)?;
  let mut writer = ZipWriter::new(file);
  let options = FileOptions::default().compression_method(CompressionMethod::Stored);

  writer.start_file("TopLevel.md", options)?;
  writer.write_all(b"page content")?;

  writer.start_file("Attachment.zip", options)?;
  writer.write_all(&create_nested_zip_bytes("nested.txt"))?;

  writer.finish()?;
  Ok(())
}

fn create_nested_zip_bytes(name: &str) -> Vec<u8> {
  let cursor = Cursor::new(Vec::new());
  let mut nested_writer = ZipWriter::new(cursor);
  let options = FileOptions::default().compression_method(CompressionMethod::Stored);

  nested_writer.start_file(name, options).unwrap();
  nested_writer.write_all(b"nested content").unwrap();

  let cursor = nested_writer.finish().unwrap();
  cursor.into_inner()
}
