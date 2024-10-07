use collab_importer::util::{unzip_from_path_or_memory, Either};
use percent_encoding::percent_decode_str;
use std::env::temp_dir;

use std::path::PathBuf;
use std::sync::Once;
use tracing_subscriber::fmt::Subscriber;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;

pub fn parse_csv(file_path: &PathBuf) -> (Vec<String>, Vec<Vec<String>>) {
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

  (csv_fields, csv_rows)
}

pub struct Cleaner(PathBuf);

impl Cleaner {
  fn new(dir: PathBuf) -> Self {
    Cleaner(dir)
  }

  fn cleanup(dir: &PathBuf) {
    let _ = std::fs::remove_dir_all(dir);
  }
}

impl Drop for Cleaner {
  fn drop(&mut self) {
    Self::cleanup(&self.0)
  }
}

pub async fn unzip_test_asset(file_name: &str) -> std::io::Result<(Cleaner, PathBuf)> {
  // Open the zip file
  let zip_file_path = PathBuf::from(format!("./tests/asset/{}.zip", file_name));
  let output_folder_path = temp_dir();
  let out_path = unzip_from_path_or_memory(Either::Left(zip_file_path), output_folder_path.clone())
    .await
    .unwrap();
  Ok((Cleaner::new(out_path.clone()), out_path))
}

pub fn setup_log() {
  static START: Once = Once::new();
  START.call_once(|| {
    std::env::set_var("RUST_LOG", "info");
    let subscriber = Subscriber::builder()
      .with_env_filter(EnvFilter::from_default_env())
      .with_ansi(true)
      .finish();
    subscriber.try_init().unwrap();
  });
}
