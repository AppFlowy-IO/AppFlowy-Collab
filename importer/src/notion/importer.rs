use crate::error::ImporterError;
use crate::imported_collab::{ImportedCollab, ImportedCollabView, ImportedType};
use anyhow::anyhow;
use base64::engine::general_purpose::URL_SAFE;
use base64::Engine;
use collab_database::database::{gen_database_view_id, Database};
use collab_database::template::csv::CSVTemplate;
use collab_document::blocks::{mention_block_data, mention_block_delta, TextDelta};
use collab_document::document::Document;
use collab_document::importer::define::{BlockType, URL_FIELD};
use collab_document::importer::md_importer::MDImporter;
use collab_entity::CollabType;
use fancy_regex::Regex;
use markdown::mdast::Node;
use markdown::{to_mdast, ParseOptions};
use percent_encoding::{percent_decode_str, utf8_percent_encode, NON_ALPHANUMERIC};
use serde::Serialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;

use std::path::{Path, PathBuf};
use tokio::io::{AsyncReadExt, BufReader};
use tracing::{error, warn};
use walkdir::{DirEntry, WalkDir};

#[derive(Debug, Clone, Serialize)]
pub struct NotionView {
  pub notion_name: String,
  pub notion_id: String,
  pub notion_file: NotionFile,
  pub object_id: String,
  pub workspace_id: String,
  pub children: Vec<NotionView>,
  pub external_links: Vec<Vec<ExternalLink>>,
  pub host: String,
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
  /// Recursively collect all the files that need to be uploaded.
  /// It will include the current view's file and all its children's files.
  pub fn get_upload_files_recursively(&self) -> Vec<(String, Vec<PathBuf>)> {
    let mut files = vec![];
    files.push((self.object_id.clone(), self.notion_file.upload_resources()));

    for child in &self.children {
      files.extend(child.get_upload_files_recursively());
    }
    files
  }

  /// Recursively collect all the files that need to be uploaded.
  pub fn get_payload_size_recursively(&self) -> u64 {
    let size = self.notion_file.file_size();
    self
      .children
      .iter()
      .map(|view| view.get_payload_size_recursively())
      .sum::<u64>()
      + size
  }

  /// Returns the number of CSV files in the view and its children.
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

  /// Returns the number of markdown files in the view and its children.
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

  pub fn get_external_link_notion_view(&self) -> HashMap<String, NotionView> {
    let mut linked_views = HashMap::new();
    for links in self.external_links.iter() {
      if let Some(link) = links.last() {
        if let Some(view) = self.get_view(&link.id) {
          linked_views.insert(link.id.clone(), view);
        }
      }
    }
    linked_views
  }

  /// Get the view with the given ID.
  /// It will search the view recursively.
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

  pub async fn as_document(
    &self,
    external_link_views: HashMap<String, NotionView>,
  ) -> Result<Document, ImporterError> {
    match &self.notion_file {
      NotionFile::Markdown {
        file_path,
        resources,
        ..
      } => {
        let md_importer = MDImporter::new(None);
        let content = std::fs::read_to_string(file_path)?;
        let document_data = md_importer.import(&self.object_id, content)?;
        let mut document = Document::create(&self.object_id, document_data)?;

        let parent_path = file_path.parent().unwrap();
        self.replace_link_views(&mut document, external_link_views);
        self
          .replace_resources(&self.workspace_id, &mut document, resources, parent_path)
          .await;

        Ok(document)
      },
      _ => Err(ImporterError::InvalidFileType(format!(
        "File type is not supported for document: {:?}",
        self.notion_file
      ))),
    }
  }

  async fn replace_resources(
    &self,
    workspace_id: &str,
    document: &mut Document,
    resources: &[Resource],
    parent_path: &Path,
  ) {
    if let Some(page_id) = document.get_page_id() {
      let block_ids = document.get_block_children_ids(&page_id);
      for block_id in block_ids.iter() {
        if let Some((block_type, mut block_data)) = document.get_block_data(block_id) {
          if matches!(block_type, BlockType::Image) {
            if let Some(image_url) = block_data
              .get(URL_FIELD)
              .and_then(|v| v.as_str())
              .and_then(|s| percent_decode_str(s).decode_utf8().ok())
            {
              let full_image_url = parent_path.join(image_url.to_string());
              if resources.iter().any(|r| r.contains(&full_image_url)) {
                if let Ok(file) = tokio::fs::File::open(&full_image_url).await {
                  let ext = Path::new(&full_image_url)
                    .extension()
                    .and_then(std::ffi::OsStr::to_str)
                    .unwrap_or("")
                    .to_owned();

                  let mut reader = BufReader::new(file);
                  let mut buffer = vec![0u8; 1024 * 1024];
                  let mut hasher = Sha256::new();
                  while let Ok(bytes_read) = reader.read(&mut buffer).await {
                    if bytes_read == 0 {
                      break;
                    }
                    hasher.update(&buffer[..bytes_read]);
                  }
                  let hash_result = hasher.finalize();
                  let file_id = format!("{}.{}", URL_SAFE.encode(hash_result), ext);
                  let parent_dir =
                    utf8_percent_encode(&self.object_id, NON_ALPHANUMERIC).to_string();
                  let url = format!(
                    "{}/{workspace_id}/v1/blob/{parent_dir}/{file_id}",
                    self.host
                  );
                  block_data.insert(URL_FIELD.to_string(), json!(url));
                  if let Err(err) = document.update_block(block_id, block_data) {
                    error!(
                      "Failed to update block when trying to replace image. error:{:?}",
                      err
                    );
                  }
                }
              }
            }
          }
        }
      }
    }
  }

  fn replace_link_views(
    &self,
    document: &mut Document,
    external_link_views: HashMap<String, NotionView>,
  ) {
    if let Some(page_id) = document.get_page_id() {
      // 2 Get all the block children of the page
      let block_ids = document.get_block_children_ids(&page_id);
      // 3. Get all the deltas of the block children
      for block_id in block_ids.iter() {
        // 4. Get the block type and deltas of the block
        if let Some((block_type, deltas)) = document.get_block_delta(block_id) {
          for delta in deltas {
            // 5. If the block type is Text, get the inserted text and attributes
            if let TextDelta::Inserted(_, Some(attrs)) = delta {
              // 6. If the block type is External, get the external ID and type
              if let Some(any) = attrs.get("href") {
                let delta_str = any.to_string();
                // 7. Extract the name and ID from the delta string
                if let Ok(links) = extract_external_links(&delta_str) {
                  if let Some(link) = links.last() {
                    if let Some(view) = external_link_views.get(&link.id) {
                      document.remove_block_delta(block_id);
                      if matches!(block_type, BlockType::Paragraph) {
                        let data = mention_block_data(&view.object_id, &self.object_id);
                        if let Err(err) = document.update_block(block_id, data) {
                          error!(
                            "Failed to update block when trying to replace ref link. error:{:?}",
                            err
                          );
                        }
                      } else {
                        let delta = mention_block_delta(&view.object_id);
                        if let Err(err) = document.set_block_delta(block_id, vec![delta]) {
                          error!(
                            "Failed to set block delta when trying to replace ref link. error:{:?}",
                            err
                          );
                        }
                      }
                    }
                  }
                }
              }
            }
          }
        }
      }
    }
  }

  pub async fn as_database(&self) -> Result<Database, ImporterError> {
    match &self.notion_file {
      NotionFile::CSV { file_path, .. } => {
        let content = std::fs::read_to_string(file_path)?;
        let csv_template = CSVTemplate::try_from(content)?;
        let database_view_id = gen_database_view_id();
        let database =
          Database::create_with_template(&self.object_id, &database_view_id, csv_template).await?;
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
        let document = self.as_document(HashMap::new()).await?;
        let encoded_collab = document.encode_collab()?;
        let imported_collab = ImportedCollab {
          object_id: self.object_id,
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
    size: u64,
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
  pub fn upload_resources(&self) -> Vec<PathBuf> {
    match self {
      NotionFile::Markdown { resources, .. } => resources
        .iter()
        .flat_map(|r| r.file_paths())
        .cloned()
        .collect(),
      _ => vec![],
    }
  }
  pub fn file_size(&self) -> u64 {
    match self {
      NotionFile::CSV { size, .. } => *size,
      NotionFile::CSVPart { size, .. } => *size,
      NotionFile::Markdown {
        size, resources, ..
      } => resources.iter().map(|r| r.size()).sum::<u64>() + *size,
      _ => 0,
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

fn get_file_size(path: &PathBuf) -> std::io::Result<u64> {
  let metadata = fs::metadata(path)?;
  let file_size = metadata.len();
  Ok(file_size)
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
}

#[derive(Debug, Serialize)]
pub struct ImportedView {
  pub workspace_id: String,
  pub host: String,
  pub name: String,
  pub views: Vec<NotionView>,
}

impl ImportedView {
  pub fn upload_files(&self) -> Vec<(String, Vec<PathBuf>)> {
    self
      .views
      .iter()
      .flat_map(|view| view.get_upload_files_recursively())
      .collect()
  }

  pub fn size(&self) -> usize {
    self.views.len()
      + self
        .views
        .iter()
        .map(|view| view.children.len())
        .sum::<usize>()
  }

  pub fn num_of_csv(&self) -> usize {
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
  host: String,
  workspace_id: String,
  path: PathBuf,
  name: String,
  pub views: Option<NotionView>,
}

impl NotionImporter {
  pub fn new<P: Into<PathBuf>, S: ToString>(
    file_path: P,
    workspace_id: S,
    host: String,
  ) -> Result<Self, ImporterError> {
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
      host,
      workspace_id: workspace_id.to_string(),
      path,
      name,
      views: None,
    })
  }

  pub async fn import(mut self) -> Result<ImportedView, ImporterError> {
    let views = self.collect_views().await?;
    Ok(ImportedView {
      workspace_id: self.workspace_id,
      host: self.host,
      name: self.name,
      views,
    })
  }

  async fn collect_views(&mut self) -> Result<Vec<NotionView>, ImporterError> {
    let views = WalkDir::new(&self.path)
      .max_depth(1)
      .into_iter()
      .filter_map(|e| e.ok())
      .filter_map(|entry| process_entry(&self.host, &self.workspace_id, &entry))
      .collect::<Vec<NotionView>>();

    Ok(views)
  }
}

fn collect_entry_resources(_workspace_id: &str, entry: &DirEntry) -> Vec<Resource> {
  let image_extensions = ["jpg", "jpeg", "png"];
  let file_extensions = ["zip"];

  let mut image_paths = Vec::new();
  let mut file_paths = Vec::new();

  // Walk through the directory
  WalkDir::new(entry.path())
      .max_depth(1)
      .into_iter()
      .filter_map(|e| e.ok()) // Ignore invalid entries
      .for_each(|entry| {
        let path = entry.path();
        if path.is_file() {
          if let Some(ext) = path.extension().and_then(|ext| ext.to_str()) {
            match fs::metadata(path).map(|file| file.len()) {
              Ok(len) => {
                let ext_lower = ext.to_lowercase();
                if image_extensions.contains(&ext_lower.as_str()) {
                  image_paths.push((path.to_path_buf(), len));
                } else if file_extensions.contains(&ext_lower.as_str()) {
                  file_paths.push((path.to_path_buf(), len));
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

fn process_entry(host: &str, workspace_id: &str, entry: &DirEntry) -> Option<NotionView> {
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
          resources.extend(collect_entry_resources(workspace_id, &sub_entry));
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
      // when current file is csv, which means its children are rows
      notion_file = NotionFile::CSV {
        file_path: csv_file_path,
        size: file_size,
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
        let links = extract_external_links(&link).ok()?;
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

pub fn extract_file_name(path: &str) -> Option<&str> {
  path.rsplit('/').next()
}

fn extract_external_links(path_str: &str) -> Result<Vec<ExternalLink>, ImporterError> {
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
