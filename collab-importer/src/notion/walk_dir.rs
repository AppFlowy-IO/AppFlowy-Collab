use crate::error::ImporterError;

use fancy_regex::Regex;
use markdown::mdast::Node;
use markdown::{to_mdast, ParseOptions};
use percent_encoding::percent_decode_str;

use crate::notion::file::{process_row_md_content, NotionFile, Resource};
use crate::notion::page::{ExternalLink, ExternalLinkType, ImportedRowDocument, NotionPage};
use crate::util::parse_csv;
use std::fs;
use std::path::{Component, Path, PathBuf};
use tracing::{error, warn};
use walkdir::{DirEntry, WalkDir};

pub(crate) fn get_file_size(path: &PathBuf) -> std::io::Result<u64> {
  let metadata = fs::metadata(path)?;
  let file_size = metadata.len();
  Ok(file_size)
}

pub(crate) fn collect_entry_resources(
  _workspace_id: &str,
  walk_path: &Path,
  relative_path: Option<&Path>,
) -> Vec<Resource> {
  let image_extensions = ["jpg", "jpeg", "png"];
  let file_extensions = ["zip"];

  let mut image_paths = Vec::new();
  let mut file_paths = Vec::new();

  // Walk through the directory
  WalkDir::new(walk_path)
      .max_depth(1)
      .into_iter()
      .filter_map(|e| e.ok()) // Ignore invalid entries
      .for_each(|entry| {
        let path = entry.path();
        if path.is_file() {
          if let Some(ext) = path.extension().and_then(|ext| ext.to_str()) {
            match fs::metadata(path).map(|file| file.len()) {
              Ok(len) => {
                let mut path_buf = path.to_path_buf();
                if let Some(rel_path) = relative_path {
                  if let Ok(stripped) = path.strip_prefix(rel_path) {
                    path_buf = stripped.to_path_buf();
                  }
                }
                let ext_lower = ext.to_lowercase();
                if image_extensions.contains(&ext_lower.as_str()) {
                  image_paths.push((path_buf, len));
                } else if file_extensions.contains(&ext_lower.as_str()) {
                  file_paths.push((path_buf, len));
                }
              }
              Err(err) => {
                error!("Failed to get file size: {:?}", err);
              }
            }

          }
        }});

  // Prepare the result
  let mut resources = Vec::new();
  if !image_paths.is_empty() {
    resources.push(Resource::Images { files: image_paths });
  }
  if !file_paths.is_empty() {
    resources.push(Resource::Files { files: file_paths });
  }
  resources
}

pub(crate) fn process_entry(
  host: &str,
  workspace_id: &str,
  current_entry: &DirEntry,
  include_partial_csv: bool,
) -> Option<NotionPage> {
  // Skip macOS-specific files
  let entry_name = current_entry.file_name().to_str()?;
  if entry_name == ".DS_Store" || entry_name.starts_with("__MACOSX") {
    return None;
  }

  let path = current_entry.path();
  let ext = get_file_extension(path, include_partial_csv);
  if ext.is_file() {
    // Check if there's a corresponding directory for this .md file and skip it if so
    process_file(host, workspace_id, path, ext)
  } else if path.is_dir() {
    // If the path is a directory, it should contain a file with the same name but with either a .md or .csv extension.
    // If no such file is found, the directory will be treated as a space.
    // Proceed to extract the name and ID for the directory.
    let (name, id) = name_and_id_from_path(path).ok()?;

    // Look for the corresponding .md file for this directory in the parent directory
    let parent_path = path.parent()?;
    let md_file_path = parent_path.join(format!("{}.md", entry_name));
    let all_csv_file_path = parent_path.join(format!("{}_all.csv", entry_name));
    let csv_file_path = parent_path.join(format!("{}.csv", entry_name));
    if md_file_path.exists() {
      process_md_dir(
        host,
        workspace_id,
        path,
        name,
        id,
        &md_file_path,
        include_partial_csv,
      )
    } else if all_csv_file_path.exists() {
      process_csv_dir(
        entry_name,
        host,
        workspace_id,
        name,
        id,
        parent_path,
        &all_csv_file_path,
        &csv_file_path,
      )
    } else {
      process_space_dir(host, workspace_id, name, id, path)
    }
  } else {
    None
  }
}

fn process_space_dir(
  host: &str,
  workspace_id: &str,
  name: String,
  id: Option<String>,
  path: &Path,
) -> Option<NotionPage> {
  let mut children = vec![];
  // Collect all child entries first, to sort by created time
  let entries: Vec<_> = walk_sub_dir(path);
  for sub_entry in entries {
    if let Some(child_view) = process_entry(host, workspace_id, &sub_entry, false) {
      children.push(child_view);
    }
  }

  Some(NotionPage {
    notion_name: name.clone(),
    notion_id: Some(id.unwrap_or_else(|| name.clone())),
    children,
    notion_file: NotionFile::Empty,
    external_links: vec![],
    view_id: uuid::Uuid::new_v4().to_string(),
    host: host.to_string(),
    workspace_id: workspace_id.to_string(),
    is_dir: true,
  })
}

#[allow(clippy::too_many_arguments)]
fn process_csv_dir(
  file_name: &str,
  host: &str,
  workspace_id: &str,
  name: String,
  id: Option<String>,
  parent_path: &Path,
  all_csv_file_path: &PathBuf,
  csv_file_path: &PathBuf,
) -> Option<NotionPage> {
  let mut resources = vec![];
  let file_size = get_file_size(all_csv_file_path).ok()?;
  // When the current file is a CSV, its related resources are found in the same directory.
  // We need to gather resources from this directory by iterating over the CSV file.
  // To identify which CSV file contains these resources, we must check each row
  // to see if any paths match the resource path.
  // Currently, we do this in [filter_out_resources].
  resources.extend(collect_entry_resources(workspace_id, parent_path, None));
  let mut row_documents = vec![];

  // collect all sub entries whose entries are directory
  if csv_file_path.exists() {
    let csv_file = parse_csv(csv_file_path);
    let csv_dir = parent_path.join(file_name);
    if csv_dir.exists() {
      for sub_entry in walk_sub_dir(&csv_dir) {
        if let Some(page) = process_entry(host, workspace_id, &sub_entry, true) {
          if page.children.iter().any(|c| c.notion_file.is_markdown()) {
            warn!("Only CSV file exist in the database row directory");
          }

          for row in csv_file.rows.iter() {
            // when page name equal to the first cell of the row. It means it's a database row document
            if page.notion_name.starts_with(&row[0]) {
              if let Some(file_path) = page.notion_file.imported_file_path() {
                if let Ok(md_content) = fs::read_to_string(file_path) {
                  if md_content.is_empty() {
                    continue;
                  }

                  // In Notion, each database row is represented as a markdown file.
                  // The content between the first-level heading (H1) and the second-level heading (H2)
                  // contains key-value pairs corresponding to the columns/cells of that row.
                  if process_row_md_content(md_content, file_path).is_ok() {
                    row_documents.push(ImportedRowDocument { page });
                  }
                }
              }

              break;
            }
          }
        }
      }
    }
  }

  let notion_file = NotionFile::CSV {
    file_path: all_csv_file_path.clone(),
    size: file_size,
    resources,
    row_documents,
  };

  Some(NotionPage {
    notion_name: name,
    notion_id: id,
    children: vec![],
    // when current file is csv, which means its children are rows
    notion_file,
    external_links: vec![],
    view_id: uuid::Uuid::new_v4().to_string(),
    host: host.to_string(),
    workspace_id: workspace_id.to_string(),
    is_dir: false,
  })
}

pub fn walk_sub_dir(path: &Path) -> Vec<DirEntry> {
  WalkDir::new(path)
    .sort_by_file_name()
    .max_depth(1)
    .into_iter()
    .filter_map(|e| e.ok())
    .filter(|e| e.path() != path)
    .collect()
}

fn process_md_dir(
  host: &str,
  workspace_id: &str,
  dir_path: &Path,
  name: String,
  id: Option<String>,
  md_file_path: &PathBuf,
  include_partial_csv: bool,
) -> Option<NotionPage> {
  let mut children = vec![];
  let external_links = get_md_links(md_file_path).unwrap_or_default();
  let mut resources = vec![];
  // Walk through sub-entries of the directory
  for sub_entry in walk_sub_dir(dir_path) {
    // Skip the directory itself and its corresponding .md file
    if sub_entry.path() != md_file_path {
      if let Some(child_view) = process_entry(host, workspace_id, &sub_entry, include_partial_csv) {
        children.push(child_view);
      }

      // When traversing the directory, resources like images and files
      // can be found within subdirectories of the current directory.
      resources.extend(collect_entry_resources(
        workspace_id,
        sub_entry.path(),
        None,
      ));
    }
  }

  let file_size = get_file_size(md_file_path).ok()?;
  let notion_file = NotionFile::Markdown {
    file_path: md_file_path.clone(),
    size: file_size,
    resources,
  };
  Some(NotionPage {
    notion_name: name,
    notion_id: id,
    children,
    notion_file,
    external_links,
    view_id: uuid::Uuid::new_v4().to_string(),
    host: host.to_string(),
    workspace_id: workspace_id.to_string(),
    is_dir: false,
  })
}

fn process_file(
  host: &str,
  workspace_id: &str,
  path: &Path,
  ext: FileExtension,
) -> Option<NotionPage> {
  match ext {
    FileExtension::Unknown => None,
    FileExtension::Markdown => process_md_file(host, workspace_id, path),
    FileExtension::Csv {
      include_partial_csv,
    } => process_csv_file(host, workspace_id, path, include_partial_csv),
  }
}

fn process_csv_file(
  host: &str,
  workspace_id: &str,
  path: &Path,
  include_partial_csv: bool,
) -> Option<NotionPage> {
  let file_name = path.file_name()?.to_str()?;
  // Check if a folder exists with the same name as the CSV file, excluding the "_all.csv" suffix.
  // When exporting a Notion zip file with the 'create folders for subpages' option enabled,
  // a folder with the same name as the CSV file may be generated.
  // For example, if the CSV file is named "abc_all.csv", a folder named "abc" will also be created.
  // In such cases, we should skip processing the CSV file.
  if let Some(parent) = path.parent() {
    let parent_path = parent.join(file_name.trim_end_matches("_all.csv"));
    if parent_path.is_dir() {
      return None;
    }
  }

  // Sometime, the exported csv might contains abc_all.csv or abc.csv. Just keep the abc_all.csv
  if !include_partial_csv && !file_name.ends_with("_all.csv") {
    // If the file name does not end with "_all", return None
    return None;
  }

  let mut resources = vec![];
  // When the current file is a CSV, its related resources are found in the same directory.
  // We need to gather resources from this directory by iterating over the CSV file.
  // To identify which CSV file contains these resources, we must check each row
  // to see if any paths match the resource path.
  // Currently, we do this in [filter_out_resources].
  if let Some(parent) = path.parent() {
    resources.extend(collect_entry_resources(workspace_id, parent, None));
  }

  let file_path = path.to_path_buf();
  let (name, id) = name_and_id_from_path(path).ok()?;
  let file_size = get_file_size(&file_path).ok()?;
  let notion_file = NotionFile::CSV {
    file_path,
    size: file_size,
    resources,
    row_documents: vec![],
  };

  Some(NotionPage {
    notion_name: name,
    notion_id: id,
    children: vec![],
    notion_file,
    external_links: vec![],
    view_id: uuid::Uuid::new_v4().to_string(),
    host: host.to_string(),
    workspace_id: workspace_id.to_string(),
    is_dir: false,
  })
}

fn process_md_file(host: &str, workspace_id: &str, path: &Path) -> Option<NotionPage> {
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
  if notion_file.is_csv() {
    return None;
  }
  Some(NotionPage {
    notion_name: name,
    notion_id: id,
    children: vec![],
    notion_file,
    external_links,
    view_id: uuid::Uuid::new_v4().to_string(),
    host: host.to_string(),
    workspace_id: workspace_id.to_string(),
    is_dir: false,
  })
}

// Main function to get all links from a markdown file
pub(crate) fn get_md_links(md_file_path: &Path) -> Result<Vec<Vec<ExternalLink>>, ImporterError> {
  let content = std::fs::read_to_string(md_file_path)?;
  let ast =
    to_mdast(&content, &ParseOptions::default()).map_err(ImporterError::ParseMarkdownError)?;
  let mut links = Vec::new();
  collect_links_from_node(&ast, &mut links);
  Ok(
    links
      .into_iter()
      .flat_map(|link| {
        let links = extract_external_links(&link).ok()?;
        Some(links)
      })
      .collect(),
  )
}

pub(crate) fn collect_links_from_node(node: &Node, links: &mut Vec<String>) {
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
fn link_type_from_extension(extension: Option<&str>) -> ExternalLinkType {
  match extension {
    Some("md") => ExternalLinkType::Markdown,
    Some("csv") => ExternalLinkType::CSV,
    _ => ExternalLinkType::Unknown,
  }
}

pub(crate) fn extract_external_links(path_str: &str) -> Result<Vec<ExternalLink>, ImporterError> {
  let path_str = percent_decode_str(path_str).decode_utf8()?.to_string();
  let mut result = Vec::new();
  let re = Regex::new(r"^(.*?)\s*([a-f0-9]{32})(?:\.(\w+))?$").unwrap();
  let path = Path::new(&path_str);

  for component in path.components() {
    if let Component::Normal(component_str) = component {
      if let Some(component_str) = component_str.to_str() {
        if let Ok(Some(captures)) = re.captures(component_str) {
          let name = captures.get(1).map_or("", |m| m.as_str()).to_string();
          let id = captures.get(2).map_or("", |m| m.as_str()).to_string();
          let link_type = captures
            .get(3)
            .map(|m| link_type_from_extension(Some(m.as_str())))
            .unwrap_or(ExternalLinkType::Unknown);

          result.push(ExternalLink {
            name,
            id,
            link_type,
          });
        }
      } else {
        return Err(ImporterError::Internal(anyhow::anyhow!(
          "Non-UTF8 path component"
        )));
      }
    }
  }

  Ok(result)
}

enum FileExtension {
  Unknown,
  Markdown,
  Csv { include_partial_csv: bool },
}

impl FileExtension {
  fn is_file(&self) -> bool {
    matches!(self, FileExtension::Markdown | FileExtension::Csv { .. })
  }
}

fn get_file_extension(path: &Path, include_partial_csv: bool) -> FileExtension {
  path
    .extension()
    .map_or(FileExtension::Unknown, |ext| match ext.to_str() {
      Some("md") => FileExtension::Markdown,
      Some("csv") => FileExtension::Csv {
        include_partial_csv,
      },
      _ => FileExtension::Unknown,
    })
}
fn name_and_id_from_path(path: &Path) -> Result<(String, Option<String>), ImporterError> {
  let re =
    Regex::new(r"^(.*?)(?:\s+([a-f0-9]{32}))?(?:_[a-zA-Z0-9]+)?(?:\.[a-zA-Z0-9]+)?\s*$").unwrap();

  let input = path
    .file_name()
    .and_then(|name| name.to_str())
    .ok_or(ImporterError::InvalidPathFormat)?;

  if let Ok(Some(captures)) = re.captures(input) {
    let file_name = captures
      .get(1)
      .map(|m| m.as_str().trim().to_string())
      .filter(|s| !s.is_empty())
      .ok_or(ImporterError::InvalidPathFormat)?;

    let file_id = captures.get(2).map(|m| m.as_str().to_string());
    return Ok((file_name, file_id));
  }

  // Fallback for cases where no ID is present but a valid name exists
  Err(ImporterError::InvalidPathFormat)
}

/// - If the file is a `.csv` and contains `_all`, it's considered a `CSV`.
/// - Otherwise, if it's a `.csv`, it's considered a `CSVPart`.
/// - `.md` files are classified as `Markdown`.
fn file_type_from_path(path: &Path) -> Option<NotionFile> {
  let extension = path.extension()?.to_str()?;
  let file_size = get_file_size(&path.to_path_buf()).ok()?;

  match extension {
    "md" => Some(NotionFile::Markdown {
      file_path: path.to_path_buf(),
      size: file_size,
      resources: vec![],
    }),
    "csv" => {
      let file_name = path.file_name()?.to_str()?;
      if file_name.contains("_all") {
        Some(NotionFile::CSV {
          file_path: path.to_path_buf(),
          size: file_size,
          resources: vec![],
          row_documents: vec![],
        })
      } else {
        Some(NotionFile::CSVPart {
          file_path: path.to_path_buf(),
          size: file_size,
        })
      }
    },
    _ => None,
  }
}

pub(crate) fn file_name_from_path(path: &Path) -> Result<String, ImporterError> {
  path
    .file_name()
    .ok_or_else(|| ImporterError::InvalidPath("can't get file name".to_string()))?
    .to_str()
    .ok_or_else(|| ImporterError::InvalidPath("file name is not a valid string".to_string()))
    .map(|s| s.to_string())
}

#[cfg(test)]
mod name_and_id_from_path_tests {
  use super::*;

  #[test]
  fn test_valid_path_with_single_space() {
    let path = Path::new("root 3 103d4deadd2c80b482abfc878985035f");
    let result = name_and_id_from_path(path);
    assert!(result.is_ok());
    let (name, id) = result.unwrap();
    assert_eq!(name, "root 3");
    assert_eq!(id.unwrap(), "103d4deadd2c80b482abfc878985035f");
  }

  #[test]
  fn test_valid_path_with_single_space2() {
    let path = Path::new("root 1 2 3 103d4deadd2c80b482abfc878985035f");
    let result = name_and_id_from_path(path);
    assert!(result.is_ok());
    let (name, id) = result.unwrap();
    assert_eq!(name, "root 1 2 3");
    assert_eq!(id.unwrap(), "103d4deadd2c80b482abfc878985035f");
  }

  #[test]
  fn test_valid_path_with_dashes() {
    let path = Path::new("root-2-1 103d4deadd2c8032bc32d094d8d5f41f");
    let result = name_and_id_from_path(path);
    assert!(result.is_ok());
    let (name, id) = result.unwrap();
    assert_eq!(name, "root-2-1");
    assert_eq!(id.unwrap(), "103d4deadd2c8032bc32d094d8d5f41f");
  }

  #[test]
  fn test_invalid_path_format_missing_id() {
    let path = Path::new("root-2-1");
    let (_name, id) = name_and_id_from_path(path).unwrap();
    assert!(id.is_none());
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
    assert_eq!(id.unwrap(), "103d4deadd2c8032bc32d094d8d5f41f");
  }

  #[test]
  fn test_space_name() {
    let path = Path::new("first space");
    let (name, id) = name_and_id_from_path(path).unwrap();
    assert_eq!(name, "first space");
    assert!(id.is_none());
  }

  #[test]
  fn test_file_name_with_ext() {
    let path = Path::new("My tasks 6db51a77742b4b11bedb1f0e02e27af8.md");
    let (name, id) = name_and_id_from_path(path).unwrap();
    assert_eq!(name, "My tasks");
    assert_eq!(id.unwrap(), "6db51a77742b4b11bedb1f0e02e27af8");
  }

  #[test]
  fn test_file_name_with_id() {
    let path = Path::new("space two 4331f936c1de4ec2bed58d49d9826c76  ");
    let (name, id) = name_and_id_from_path(path).unwrap();
    assert_eq!(name, "space two");
    assert_eq!(id.unwrap(), "4331f936c1de4ec2bed58d49d9826c76");
  }

  #[test]
  fn test_file_name_with_all_csv() {
    let path = Path::new("Projects 58b8977d6e4444a98ec4d64176a071e5_all.csv");
    let (name, id) = name_and_id_from_path(path).unwrap();
    assert_eq!(name, "Projects");
    assert_eq!(id.unwrap(), "58b8977d6e4444a98ec4d64176a071e5");
  }
  #[test]
  fn version_number_name_test() {
    let path = Path::new("v0 7 2 11f96b61692380489555ecb38b723e46");
    let (name, id) = name_and_id_from_path(path).unwrap();
    assert_eq!(name, "v0 7 2");
    assert_eq!(id.unwrap(), "11f96b61692380489555ecb38b723e46");
  }
}

#[cfg(test)]
mod extract_external_links_tests {
  use super::*;

  #[test]
  fn test_extract_external_links_valid_md_path() {
    // Test with a valid path containing an external link
    let path_str = "folder/Marketing_campaign e445ee1fb7ff4591be2de17d906df97e.md";
    let result = extract_external_links(path_str);

    assert!(result.is_ok());
    let links = result.unwrap();
    assert_eq!(links.len(), 1);

    let link = &links[0];
    assert_eq!(link.name, "Marketing_campaign");
    assert_eq!(link.id, "e445ee1fb7ff4591be2de17d906df97e");
    assert_eq!(link.link_type, ExternalLinkType::Markdown);
  }

  #[test]
  fn test_extract_external_links_valid_csv_path() {
    // Test with a valid path containing an external link
    let path_str = "folder/Marketing_campaign e445ee1fb7ff4591be2de17d906df97e.csv";
    let result = extract_external_links(path_str);

    assert!(result.is_ok());
    let links = result.unwrap();
    assert_eq!(links.len(), 1);

    let link = &links[0];
    assert_eq!(link.name, "Marketing_campaign");
    assert_eq!(link.id, "e445ee1fb7ff4591be2de17d906df97e");
    assert_eq!(link.link_type, ExternalLinkType::CSV);
  }

  #[test]
  fn test_extract_external_links_multiple_components() {
    // Test with a path containing multiple components
    let path_str = "folder/Research_study e445ee1fb7ff4591be2de17d906df97e/file_2 a8e534ad763040029d0feb27fdb1820d.md";
    let result = extract_external_links(path_str);

    assert!(result.is_ok());
    let links = result.unwrap();
    assert_eq!(links.len(), 2);

    assert_eq!(links[0].name, "Research_study");
    assert_eq!(links[0].id, "e445ee1fb7ff4591be2de17d906df97e");
    assert_eq!(links[0].link_type, ExternalLinkType::Unknown);

    assert_eq!(links[1].name, "file_2");
    assert_eq!(links[1].id, "a8e534ad763040029d0feb27fdb1820d");
    assert_eq!(links[1].link_type, ExternalLinkType::Markdown);
  }
}
