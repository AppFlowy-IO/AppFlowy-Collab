use importer::notion::{NotionImporter, NotionView};
use nanoid::nanoid;
use std::fs::{create_dir_all, File};
use std::io::copy;
use std::path::{Path, PathBuf};
use zip::ZipArchive;

#[tokio::test]
async fn test_importer() {
  let parent_dir = nanoid!(6);
  let (_cleaner, file_path) = unzip("import_test", &parent_dir).unwrap();
  let importer = NotionImporter::new(&file_path).unwrap();
  let imported_view = importer.import().await.unwrap();
  assert_eq!(!imported_view.views.is_empty());
  assert_eq!(imported_view.name, "import_test");

  /*
  - Root2:MD
    - root2-link:MD
  - Home:MD
    - Home views:MD
    - My tasks:MD
  - Root:MD
    - root-2:MD
      - root-2-1:MD
        - root-2-database:MD
          - Untitled:MD
          - Untitled:MD
          - Untitled:MD
        - root-2-database:CSV
    - root-1:MD
      - root-1-1:MD
    - root 3:MD
      - root 3 1:MD
    */
  for view in imported_view.views {
    print_view(&view, 0);
  }
}

fn print_view(view: &NotionView, depth: usize) {
  let indent = "  ".repeat(depth);
  println!("{}- {}:{:?}", indent, view.notion_name, view.file_type);

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
  let zip_file_path = format!("./tests/{}.zip", file_name);
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
