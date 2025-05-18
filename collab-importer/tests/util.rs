use std::env::temp_dir;

use async_zip::base::read::stream::ZipFileReader;
use collab_importer::zip_tool::async_zip::async_unzip;
use collab_importer::zip_tool::sync_zip::sync_unzip;
use std::path::PathBuf;
use std::sync::Once;
use tokio::io::BufReader;
use tokio_util::compat::TokioAsyncReadCompatExt;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt::Subscriber;
use tracing_subscriber::util::SubscriberInitExt;

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

pub async fn sync_unzip_asset(file_name: &str) -> std::io::Result<(Cleaner, PathBuf)> {
  let zip_file_path = PathBuf::from(format!("./tests/asset/{}.zip", file_name));
  if !zip_file_path.exists() {
    panic!("File not found: {:?}", zip_file_path);
  }
  let file_name = zip_file_path
    .file_stem()
    .unwrap()
    .to_str()
    .unwrap()
    .to_string();

  let output_folder_path = temp_dir().join(uuid::Uuid::new_v4().to_string());
  // let output_folder_path = std::env::current_dir()
  //   .unwrap()
  //   .join(uuid::Uuid::new_v4().to_string());
  tokio::fs::create_dir_all(&output_folder_path).await?;

  let start = std::time::Instant::now();
  let unzip_file_path = tokio::task::spawn_blocking(move || {
    sync_unzip(zip_file_path, output_folder_path.clone(), Some(file_name))
      .unwrap()
      .unzip_dir
  })
  .await
  .unwrap();

  println!("sync_unzip_asset took: {:?}", start.elapsed());

  Ok((Cleaner::new(unzip_file_path.clone()), unzip_file_path))
}

pub async fn async_unzip_asset(file_name: &str) -> std::io::Result<(Cleaner, PathBuf)> {
  setup_log();
  let zip_file_path = PathBuf::from(format!("./tests/asset/{}.zip", file_name));
  let output_folder_path = temp_dir().join(uuid::Uuid::new_v4().to_string());
  tokio::fs::create_dir_all(&output_folder_path).await?;

  let file_name = zip_file_path
    .file_stem()
    .unwrap()
    .to_str()
    .unwrap()
    .to_string();
  let file = tokio::fs::File::open(&zip_file_path).await.unwrap();
  let reader = BufReader::new(file).compat();
  let zip_reader = ZipFileReader::new(reader);
  let unzip_file_path = async_unzip(zip_reader, output_folder_path, Some(file_name))
    .await
    .unwrap()
    .unzip_dir_path;
  Ok((Cleaner::new(unzip_file_path.clone()), unzip_file_path))
}

pub fn setup_log() {
  static START: Once = Once::new();
  START.call_once(|| {
    let level = "trace";
    let mut filters = vec![];
    filters.push(format!("collab_importer={}", level));
    unsafe {
      std::env::set_var("RUST_LOG", filters.join(","));
    }
    let subscriber = Subscriber::builder()
      .with_env_filter(EnvFilter::from_default_env())
      .with_ansi(true)
      .finish();
    subscriber.try_init().unwrap();
  });
}
