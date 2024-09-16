use crate::error::ImporterError;
use serde::Serialize;
use std::path::{Path, PathBuf};
use walkdir::{DirEntry, WalkDir};

#[derive(Debug, Serialize)]
pub struct NotionView {
  pub name: String,
  pub id: String,
  pub children: Vec<NotionView>,
  pub file_type: FileType,
  pub file_path: Option<PathBuf>,
}

#[derive(Debug, Serialize)]
pub enum FileType {
  CSV,
  MD,
}

#[derive(Debug, Serialize)]
pub struct ImportedView {
  pub name: String,
  pub views: Vec<NotionView>,
}

#[derive(Debug)]
pub struct NotionImporter {
  path: PathBuf,
  name: String,
  pub views: Option<NotionView>,
}

impl NotionImporter {
  pub fn new<P: Into<PathBuf>>(file_path: P) -> Result<Self, ImporterError> {
    let path = file_path.into();
    if !path.exists() {
      return Err(ImporterError::InvalidPath(
        "Path: does not exist".to_string(),
      ));
    }

    let name = file_name_from_path(&path).unwrap_or_else(|_| {
      let now = chrono::Utc::now();
      format!("import-{}", now.format("%Y-%m-%d %H:%M"))
    });

    Ok(Self {
      path,
      name,
      views: None,
    })
  }

  pub async fn import(mut self) -> Result<ImportedView, ImporterError> {
    let views = self.collect_views().await?;
    Ok(ImportedView {
      name: self.name,
      views,
    })
  }

  async fn collect_views(&mut self) -> Result<Vec<NotionView>, ImporterError> {
    let views = WalkDir::new(&self.path)
      .max_depth(1)
      .into_iter()
      .filter_map(|e| e.ok())
      .filter_map(process_entry)
      .collect::<Vec<NotionView>>();

    Ok(views)
  }
}
fn process_entry(entry: DirEntry) -> Option<NotionView> {
  let path = entry.path();

  if path.is_file() && is_valid_file(path) {
    // Check if there's a corresponding directory for this .md file and skip it if so
    if let Some(parent) = path.parent() {
      let file_stem = path.file_stem()?.to_str()?;
      let corresponding_dir = parent.join(file_stem);
      if corresponding_dir.is_dir() {
        return None; // Skip .md file if there's a corresponding directory
      }
    }

    // Process the file normally if it doesn't correspond to a directory
    let (name, id) = name_and_id_from_path(path).ok()?;
    let file_type = get_file_type(path)?;
    return Some(NotionView {
      name,
      id,
      children: vec![],
      file_type,
      file_path: Some(path.to_path_buf()),
    });
  } else if path.is_dir() {
    // Extract name and ID for the directory
    let (name, id) = name_and_id_from_path(path).ok()?;
    let mut children = vec![];

    // Look for the corresponding .md file for this directory in the parent directory
    let dir_name = path.file_name()?.to_str()?;
    let parent_path = path.parent()?;
    let md_file_path = parent_path.join(format!("{}.md", dir_name));
    let file_path = if md_file_path.exists() {
      Some(md_file_path.clone()) // Use .md file as the directory's file_path
    } else {
      None // No corresponding .md file, so no special file_path
    };

    // Walk through sub-entries of the directory
    for sub_entry in WalkDir::new(path)
      .max_depth(1)
      .into_iter()
      .filter_map(|e| e.ok())
    {
      // Skip the directory itself and its corresponding .md file
      if sub_entry.path() != path && sub_entry.path() != md_file_path {
        if let Some(child_view) = process_entry(sub_entry) {
          children.push(child_view);
        }
      }
    }

    return Some(NotionView {
      name,
      id,
      children,
      file_type: FileType::MD,
      file_path,
    });
  }
  None
}

fn is_valid_file(path: &Path) -> bool {
  path
    .extension()
    .map_or(false, |ext| ext == "md" || ext == "csv")
}

fn name_and_id_from_path(path: &Path) -> Result<(String, String), ImporterError> {
  // Extract the file name from the path
  let file_name = path
    .file_name()
    .and_then(|name| name.to_str())
    .ok_or(ImporterError::InvalidPathFormat)?;

  // Split the file name into two parts: name and ID
  let mut parts = file_name.rsplitn(2, ' ');
  let id = parts
    .next()
    .ok_or(ImporterError::InvalidPathFormat)?
    .to_string();

  // Remove the file extension from the ID if it's present
  let id = Path::new(&id)
    .file_stem()
    .ok_or(ImporterError::InvalidPathFormat)?
    .to_string_lossy()
    .to_string();

  let name = parts
    .next()
    .ok_or(ImporterError::InvalidPathFormat)?
    .to_string();

  if name.is_empty() || id.is_empty() {
    return Err(ImporterError::InvalidPathFormat);
  }

  Ok((name, id))
}

fn get_file_type(path: &Path) -> Option<FileType> {
  match path.extension()?.to_str()? {
    "md" => Some(FileType::MD),
    "csv" => Some(FileType::CSV),
    _ => None,
  }
}

fn file_name_from_path(path: &Path) -> Result<String, ImporterError> {
  path
    .file_name()
    .ok_or_else(|| ImporterError::InvalidPath("can't get file name".to_string()))?
    .to_str()
    .ok_or_else(|| ImporterError::InvalidPath("file name is not a valid string".to_string()))
    .map(|s| s.to_string())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_valid_path_with_single_space() {
    let path = Path::new("root 3 103d4deadd2c80b482abfc878985035f");
    let result = name_and_id_from_path(path);
    assert!(result.is_ok());
    let (name, id) = result.unwrap();
    assert_eq!(name, "root 3");
    assert_eq!(id, "103d4deadd2c80b482abfc878985035f");
  }

  #[test]
  fn test_valid_path_with_single_space2() {
    let path = Path::new("root 1 2 3 103d4deadd2c80b482abfc878985035f");
    let result = name_and_id_from_path(path);
    assert!(result.is_ok());
    let (name, id) = result.unwrap();
    assert_eq!(name, "root 1 2 3");
    assert_eq!(id, "103d4deadd2c80b482abfc878985035f");
  }

  #[test]
  fn test_valid_path_with_dashes() {
    let path = Path::new("root-2-1 103d4deadd2c8032bc32d094d8d5f41f");
    let result = name_and_id_from_path(path);
    assert!(result.is_ok());
    let (name, id) = result.unwrap();
    assert_eq!(name, "root-2-1");
    assert_eq!(id, "103d4deadd2c8032bc32d094d8d5f41f");
  }

  #[test]
  fn test_invalid_path_format_missing_id() {
    let path = Path::new("root-2-1");
    let result = name_and_id_from_path(path);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().to_string(), "Invalid path format");
  }

  #[test]
  fn test_invalid_path_format_missing_name() {
    let path = Path::new(" 103d4deadd2c8032bc32d094d8d5f41f");
    let result = name_and_id_from_path(path);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().to_string(), "Invalid path format");
  }

  #[test]
  fn test_path_with_multiple_spaces_in_name() {
    let path = Path::new("root with spaces 103d4deadd2c8032bc32d094d8d5f41f");
    let result = name_and_id_from_path(path);
    assert!(result.is_ok());
    let (name, id) = result.unwrap();
    assert_eq!(name, "root with spaces");
    assert_eq!(id, "103d4deadd2c8032bc32d094d8d5f41f");
  }

  #[test]
  fn test_valid_path_with_no_spaces_in_name() {
    let path = Path::new("rootname103d4deadd2c8032bc32d094d8d5f41f");
    let result = name_and_id_from_path(path);
    assert!(result.is_err());
  }
}
