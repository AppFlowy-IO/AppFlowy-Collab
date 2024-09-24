use crate::error::ImporterError;

use anyhow::anyhow;
use fancy_regex::Regex;
use markdown::mdast::Node;
use markdown::{to_mdast, ParseOptions};
use percent_encoding::percent_decode_str;

use crate::notion::page::{ExternalLink, ExternalLinkType, NotionView};
use std::fs;
use std::path::{Path, PathBuf};

use crate::notion::file::{NotionFile, Resource};
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
) -> Option<NotionView> {
  let path = current_entry.path();

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
      object_id: uuid::Uuid::new_v4().to_string(),
      host: host.to_string(),
      workspace_id: workspace_id.to_string(),
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

    let mut resources = vec![];
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
          if let Some(child_view) = process_entry(host, workspace_id, &sub_entry) {
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

      let file_size = get_file_size(&md_file_path).ok()?;
      notion_file = NotionFile::Markdown {
        file_path: md_file_path,
        size: file_size,
        resources,
      }
    } else if csv_file_path.exists() {
      let file_size = get_file_size(&csv_file_path).ok()?;
      // When the current file is a CSV, its related resources are found in the same directory.
      // We need to gather resources from this directory by iterating over the CSV file.
      // To identify which CSV file contains these resources, we must check each row
      // to see if any paths match the resource path.
      // Currently, we do this in [filter_out_resources].
      resources.extend(collect_entry_resources(
        workspace_id,
        parent_path,
        None,
        // parent_path.parent(),
      ));

      // when current file is csv, which means its children are rows
      notion_file = NotionFile::CSV {
        file_path: csv_file_path,
        size: file_size,
        resources,
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
      object_id: uuid::Uuid::new_v4().to_string(),
      host: host.to_string(),
      workspace_id: workspace_id.to_string(),
    });
  }
  None
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

pub(crate) fn extract_external_links(path_str: &str) -> Result<Vec<ExternalLink>, ImporterError> {
  let path_str = percent_decode_str(path_str).decode_utf8()?.to_string();
  let mut result = Vec::new();
  let re = Regex::new(r"^(.*?)\s([0-9a-fA-F]{32})(?:_all)?(?:\.(\w+))?$").unwrap();
  let path = Path::new(&path_str);
  for component in path.components() {
    if let Some(component_str) = component.as_os_str().to_str() {
      if let Ok(Some(captures)) = re.captures(component_str) {
        let link = || {
          let name = captures.get(1)?.as_str().to_string();
          let id = captures.get(2)?.as_str().to_string();
          let link_type = match captures.get(3) {
            None => ExternalLinkType::Unknown,
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

pub(crate) fn is_valid_file(path: &Path) -> bool {
  path
    .extension()
    .map_or(false, |ext| ext == "md" || ext == "csv")
}

pub(crate) fn name_and_id_from_path(path: &Path) -> Result<(String, String), ImporterError> {
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

fn link_type_from_str(file_type: &str) -> ExternalLinkType {
  match file_type {
    "md" => ExternalLinkType::Markdown,
    "csv" => ExternalLinkType::CSV,
    _ => ExternalLinkType::Unknown,
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
