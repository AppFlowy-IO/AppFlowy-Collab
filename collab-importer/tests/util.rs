use collab_importer::notion::page::NotionView;
use percent_encoding::percent_decode_str;
use std::fs::{create_dir_all, File};
use std::io::copy;
use std::path::{Path, PathBuf};
use std::sync::Once;
use tracing_subscriber::fmt::Subscriber;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;
use zip::ZipArchive;

pub fn print_view(view: &NotionView, depth: usize) {
  let indent = "  ".repeat(depth);
  println!("{}- {}:{:?}", indent, view.notion_name, view.notion_file);

  for child in &view.children {
    print_view(child, depth + 1);
  }
}

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

pub fn unzip(file_name: &str, parent_dir: &str) -> std::io::Result<(Cleaner, PathBuf)> {
  // Open the zip file
  let zip_file_path = format!("./tests/asset/{}.zip", file_name);
  let reader = File::open(zip_file_path)?;
  let output_folder_path = format!("./tests/temp/{}", parent_dir);
  let mut archive = ZipArchive::new(reader)?;
  for i in 0..archive.len() {
    let mut file = archive.by_index(i)?;
    let outpath = Path::new(&output_folder_path).join(file.mangled_name());

    if file.name().ends_with('/') {
      create_dir_all(&outpath)?;
    } else {
      if let Some(p) = outpath.parent() {
        if !p.exists() {
          create_dir_all(p)?;
        }
      }
      let mut outfile = File::create(&outpath)?;
      copy(&mut file, &mut outfile)?;
    }
  }
  let path = format!("{}/{}", output_folder_path, file_name);
  Ok((
    Cleaner::new(PathBuf::from(output_folder_path)),
    PathBuf::from(path),
  ))
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
