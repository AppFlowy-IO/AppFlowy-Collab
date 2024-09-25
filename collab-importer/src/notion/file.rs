use serde::Serialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize)]
pub enum LinkType {
  Unknown,
  CSV,
  Markdown,
}

#[derive(Debug, Default, Clone, Eq, PartialEq, Serialize)]
pub enum NotionFile {
  #[default]
  Unknown,
  CSV {
    file_path: PathBuf,
    size: u64,
    resources: Vec<Resource>,
  },
  CSVPart {
    file_path: PathBuf,
    size: u64,
  },
  Markdown {
    file_path: PathBuf,
    size: u64,
    resources: Vec<Resource>,
  },
}

impl NotionFile {
  pub fn is_markdown(&self) -> bool {
    matches!(self, NotionFile::Markdown { .. })
  }

  pub fn is_csv_all(&self) -> bool {
    matches!(self, NotionFile::CSV { .. })
  }
  pub fn imported_file_path(&self) -> Option<&PathBuf> {
    match self {
      NotionFile::CSV { file_path, .. } => Some(file_path),
      NotionFile::Markdown { file_path, .. } => Some(file_path),
      _ => None,
    }
  }
  pub fn upload_files(&self) -> Vec<PathBuf> {
    match self {
      NotionFile::Markdown { resources, .. } => resources
        .iter()
        .flat_map(|r| r.file_paths())
        .cloned()
        .collect(),
      NotionFile::CSV { resources, .. } => resources
        .iter()
        .flat_map(|r| r.file_paths())
        .cloned()
        .collect(),
      _ => vec![],
    }
  }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
pub enum Resource {
  Images { files: Vec<(PathBuf, u64)> },
  Files { files: Vec<(PathBuf, u64)> },
}

impl Resource {
  pub fn file_paths(&self) -> Vec<&PathBuf> {
    match self {
      Resource::Images { files } => files.iter().map(|(path, _)| path).collect(),
      Resource::Files { files } => files.iter().map(|(path, _)| path).collect(),
    }
  }
  pub fn size(&self) -> u64 {
    match self {
      Resource::Images { files } => files.iter().map(|(_, size)| *size).sum(),
      Resource::Files { files } => files.iter().map(|(_, size)| *size).sum(),
    }
  }
  pub fn contains(&self, path: &PathBuf) -> bool {
    match self {
      Resource::Images { files } => files.iter().any(|(file_path, _)| file_path == path),
      Resource::Files { files } => files.iter().any(|(file_path, _)| file_path == path),
    }
  }
}
