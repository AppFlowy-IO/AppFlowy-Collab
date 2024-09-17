use importer::notion::NotionView;
use std::fs::{create_dir_all, File};
use std::io::copy;
use std::path::{Path, PathBuf};
use zip::ZipArchive;

pub fn print_view(view: &NotionView, depth: usize) {
  let indent = "  ".repeat(depth);
  println!("{}- {}:{:?}", indent, view.notion_name, view.notion_file);

  for child in &view.children {
    print_view(child, depth + 1);
  }
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
