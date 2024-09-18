use crate::error::ImporterError;
use crate::imported_collab::{ImportedCollab, ImportedCollabView, ImportedType};
use anyhow::anyhow;
use collab_database::database::{gen_database_id, gen_database_view_id, Database};
use collab_database::template::csv::CSVTemplate;
use collab_document::document::{gen_document_id, Document};
use collab_document::importer::md_importer::MDImporter;
use collab_entity::CollabType;
use fancy_regex::Regex;
use markdown::mdast::Node;
use markdown::{to_mdast, ParseOptions};
use percent_encoding::percent_decode_str;
use serde::Serialize;

use std::path::{Path, PathBuf};
use tracing::warn;
use walkdir::{DirEntry, WalkDir};

#[derive(Debug, Clone, Serialize)]
pub struct NotionView {
  pub notion_name: String,
  pub notion_id: String,
  pub children: Vec<NotionView>,
  pub notion_file: NotionFile,
  pub external_links: Vec<Vec<ExternalLink>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExternalLink {
  pub id: String,
  pub name: String,
  pub link_type: LinkType,
}

#[derive(Debug, Clone, Serialize)]
pub enum LinkType {
  Unknown,
  CSV,
  Markdown,
}

impl NotionView {
  pub fn num_of_csv(&self) -> usize {
    self
      .children
      .iter()
      .map(|view| view.num_of_csv())
      .sum::<usize>()
      + if matches!(self.notion_file, NotionFile::CSV { .. }) {
        1
      } else {
        0
      }
  }

  pub fn num_of_markdown(&self) -> usize {
    self
      .children
      .iter()
      .map(|view| view.num_of_markdown())
      .sum::<usize>()
      + if matches!(self.notion_file, NotionFile::Markdown { .. }) {
        1
      } else {
        0
      }
  }

  pub fn get_external_link_notion_view(&self) -> Vec<NotionView> {
    let mut linked_views = vec![];
    for links in self.external_links.iter() {
      if let Some(link) = links.last() {
        if let Some(view) = self.get_view(&link.id) {
          linked_views.push(view);
        }
      }
    }
    linked_views
  }

  pub fn get_view(&self, id: &str) -> Option<NotionView> {
    fn search_view(views: &[NotionView], id: &str) -> Option<NotionView> {
      for view in views {
        if view.notion_id == id {
          return Some(view.clone());
        }
        if let Some(child_view) = search_view(&view.children, id) {
          return Some(child_view);
        }
      }
      None
    }

    search_view(&self.children, id)
  }

  pub fn get_linked_views(&self) -> Vec<NotionView> {
    let mut linked_views = vec![];
    for link in &self.external_links {
      for external_link in link {
        if let Some(view) = self.get_view(&external_link.id) {
          linked_views.push(view);
        }
      }
    }
    linked_views
  }

  pub async fn as_document(&self, document_id: &str) -> Result<Document, ImporterError> {
    match &self.notion_file {
      NotionFile::Markdown { file_path } => {
        let md_importer = MDImporter::new(None);
        let content = std::fs::read_to_string(file_path)?;
        let document_data = md_importer.import(document_id, content)?;
        let document = Document::create(document_id, document_data)?;
        Ok(document)
      },
      _ => Err(ImporterError::InvalidFileType(format!(
        "File type is not supported for document: {:?}",
        self.notion_file
      ))),
    }
  }

  pub async fn as_database(&self) -> Result<Database, ImporterError> {
    match &self.notion_file {
      NotionFile::CSV { file_path } => {
        let content = std::fs::read_to_string(file_path)?;
        let csv_template = CSVTemplate::try_from(content)?;
        let database_id = gen_database_id();
        let database_view_id = gen_database_view_id();
        let database =
          Database::create_with_template(&database_id, &database_view_id, csv_template).await?;
        Ok(database)
      },
      _ => Err(ImporterError::InvalidFileType(format!(
        "File type is not supported for database: {:?}",
        self.notion_file
      ))),
    }
  }

  pub async fn try_into_collab(self) -> Result<ImportedCollabView, ImporterError> {
    match self.notion_file {
      NotionFile::CSV { .. } => {
        let database = self.as_database().await?;
        let imported_collabs = database
          .encode_database_collabs()
          .await?
          .into_collabs()
          .into_iter()
          .map(|collab_info| ImportedCollab {
            object_id: collab_info.object_id,
            collab_type: collab_info.collab_type,
            encoded_collab: collab_info.encoded_collab,
          })
          .collect::<Vec<_>>();

        Ok(ImportedCollabView {
          name: self.notion_name,
          imported_type: ImportedType::Database,
          collabs: imported_collabs,
        })
      },
      NotionFile::Markdown { .. } => {
        let document_id = gen_document_id();
        let document = self.as_document(&document_id).await?;
        let encoded_collab = document.encode_collab()?;
        let imported_collab = ImportedCollab {
          object_id: document_id,
          collab_type: CollabType::Document,
          encoded_collab,
        };
        Ok(ImportedCollabView {
          name: self.notion_name,
          imported_type: ImportedType::Document,
          collabs: vec![imported_collab],
        })
      },
      _ => Err(ImporterError::InvalidFileType(format!(
        "File type is not supported for collab: {:?}",
        self.notion_file
      ))),
    }
  }
}

#[derive(Debug, Default, Clone, Eq, PartialEq, Serialize)]
pub enum NotionFile {
  #[default]
  Unknown,
  CSV {
    file_path: PathBuf,
  },
  CSVPart {
    file_path: PathBuf,
  },
  Markdown {
    file_path: PathBuf,
  },
}
impl NotionFile {
  pub fn is_markdown(&self) -> bool {
    matches!(self, NotionFile::Markdown { .. })
  }

  pub fn is_csv_all(&self) -> bool {
    matches!(self, NotionFile::CSV { .. })
  }
  pub fn file_path(&self) -> Option<&PathBuf> {
    match self {
      NotionFile::CSV { file_path } => Some(file_path),
      NotionFile::Markdown { file_path } => Some(file_path),
      _ => None,
    }
  }
}

#[derive(Debug, Serialize)]
pub struct ImportedView {
  pub name: String,
  pub views: Vec<NotionView>,
}

impl ImportedView {
  pub fn num_of_csv(&self) -> usize {
    for view in self.views.iter() {
      let a = view.num_of_csv();
      println!("aaaaa: {}", a);
    }
    self
      .views
      .iter()
      .map(|view| view.num_of_csv())
      .sum::<usize>()
  }

  pub fn num_of_markdown(&self) -> usize {
    self
      .views
      .iter()
      .map(|view| view.num_of_markdown())
      .sum::<usize>()
  }
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
        return None; // Skip .md or .csv file if there's a corresponding directory
      }
    }

    // Process the file normally if it doesn't correspond to a directory
    let (name, id) = name_and_id_from_path(path).ok()?;
    let notion_file = file_type_from_path(path)?;
    let mut external_links = vec![];
    if notion_file.is_markdown() {
      external_links = get_md_links(path).unwrap_or_default();
    }

    // If the file is CSV, then it should be handled later.
    if notion_file.is_csv_all() {
      return None;
    }
    return Some(NotionView {
      notion_name: name,
      notion_id: id,
      children: vec![],
      notion_file,
      external_links,
    });
  } else if path.is_dir() {
    // When the path is directory, which means it should has a file with the same name but with .md
    // or .csv extension.

    // Extract name and ID for the directory
    let (name, id) = name_and_id_from_path(path).ok()?;
    let mut children = vec![];

    // Look for the corresponding .md file for this directory in the parent directory
    let dir_name = path.file_name()?.to_str()?;
    let parent_path = path.parent()?;

    let notion_file: NotionFile;
    let mut external_links = vec![];
    let md_file_path = parent_path.join(format!("{}.md", dir_name));
    let csv_file_path = parent_path.join(format!("{}_all.csv", dir_name));

    if md_file_path.exists() {
      external_links = get_md_links(&md_file_path).unwrap_or_default();
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
      notion_file = NotionFile::Markdown {
        file_path: md_file_path,
      }
    } else if csv_file_path.exists() {
      // when current file is csv, which means its children are rows
      notion_file = NotionFile::CSV {
        file_path: csv_file_path,
      }
    } else {
      warn!("No corresponding .md file found for directory: {:?}", path);
      return None;
    }

    return Some(NotionView {
      notion_name: name,
      notion_id: id,
      children,
      notion_file,
      external_links,
    });
  }
  None
}

// Main function to get all links from a markdown file
fn get_md_links(md_file_path: &Path) -> Result<Vec<Vec<ExternalLink>>, ImporterError> {
  let content = std::fs::read_to_string(md_file_path)?;
  let ast =
    to_mdast(&content, &ParseOptions::default()).map_err(ImporterError::ParseMarkdownError)?;
  let mut links = Vec::new();
  collect_links_from_node(&ast, &mut links);
  Ok(
    links
      .into_iter()
      .flat_map(|link| {
        let str = percent_decode_str(&link).decode_utf8().ok()?.to_string();
        let links = extract_name_id(&str).ok()?;
        Some(links)
      })
      .collect(),
  )
}

fn collect_links_from_node(node: &Node, links: &mut Vec<String>) {
  match node {
    // For standard links, push the URL
    Node::Link(link) => {
      links.push(link.url.clone());
    },
    // For link references, push the identifier
    Node::LinkReference(link_ref) => {
      links.push(link_ref.identifier.clone());
    },
    // If the node is a container, recurse into its children
    Node::Root(root) => {
      for child in &root.children {
        collect_links_from_node(child, links);
      }
    },
    Node::Paragraph(paragraph) => {
      for child in &paragraph.children {
        collect_links_from_node(child, links);
      }
    },
    _ => {},
  }
}

fn extract_name_id(path_str: &str) -> Result<Vec<ExternalLink>, ImporterError> {
  let mut result = Vec::new();
  let re = Regex::new(r"^(.*?)\s([0-9a-fA-F]{32})(?:_all)?(?:\.(\w+))?$").unwrap();
  let path = Path::new(path_str);
  for component in path.components() {
    if let Some(component_str) = component.as_os_str().to_str() {
      if let Ok(Some(captures)) = re.captures(component_str) {
        let link = || {
          let name = captures.get(1)?.as_str().to_string();
          let id = captures.get(2)?.as_str().to_string();
          let link_type = match captures.get(3) {
            None => LinkType::Unknown,
            Some(s) => link_type_from_str(s.as_str()),
          };
          Some(ExternalLink {
            id,
            name,
            link_type,
          })
        };
        if let Some(link) = link() {
          result.push(link);
        }
      }
    } else {
      return Err(ImporterError::Internal(anyhow!("Non-UTF8 path component")));
    }
  }

  Ok(result)
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
/// - If the file is a `.csv` and contains `_all`, it's considered a `CSV`.
/// - Otherwise, if it's a `.csv`, it's considered a `CSVPart`.
/// - `.md` files are classified as `Markdown`.
fn file_type_from_path(path: &Path) -> Option<NotionFile> {
  let extension = path.extension()?.to_str()?;

  match extension {
    "md" => Some(NotionFile::Markdown {
      file_path: path.to_path_buf(),
    }),
    "csv" => {
      let file_name = path.file_name()?.to_str()?;
      if file_name.contains("_all") {
        Some(NotionFile::CSV {
          file_path: path.to_path_buf(),
        })
      } else {
        Some(NotionFile::CSVPart {
          file_path: path.to_path_buf(),
        })
      }
    },
    _ => None,
  }
}

fn link_type_from_str(file_type: &str) -> LinkType {
  match file_type {
    "md" => LinkType::Markdown,
    "csv" => LinkType::CSV,
    _ => LinkType::Unknown,
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
