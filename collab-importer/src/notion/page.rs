use crate::error::ImporterError;
use crate::imported_collab::{ImportType, ImportedCollab, ImportedCollabInfo};

use collab_database::database::{gen_database_view_id, Database};
use collab_database::template::csv::CSVTemplate;
use collab_document::blocks::{mention_block_data, mention_block_delta, TextDelta};
use collab_document::document::Document;
use collab_document::importer::define::{BlockType, URL_FIELD};
use collab_document::importer::md_importer::MDImporter;
use collab_entity::CollabType;

use percent_encoding::percent_decode_str;
use serde::Serialize;
use serde_json::json;
use std::collections::{HashMap, HashSet};

use crate::notion::file::{NotionFile, Resource};
use crate::notion::walk_dir::extract_external_links;
use crate::util::{upload_file_url, FileId};
use async_recursion::async_recursion;
use std::path::{Path, PathBuf};
use tracing::error;

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

impl NotionView {
  /// Returns the files that need to be uploaded for current view.
  pub fn get_upload_files(&self) -> Vec<PathBuf> {
    self.notion_file.upload_resources()
  }

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
  pub fn get_file_size_recursively(&self) -> u64 {
    let size = self.notion_file.file_size();
    self
      .children
      .iter()
      .map(|view| view.get_file_size_recursively())
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
  ) -> Result<(Document, Vec<String>), ImporterError> {
    match &self.notion_file {
      NotionFile::Markdown { file_path, .. } => {
        let mut resource_paths = self.notion_file.upload_resources();
        let md_importer = MDImporter::new(None);
        let content = std::fs::read_to_string(file_path)?;
        let document_data = md_importer.import(&self.object_id, content)?;
        let mut document = Document::create(&self.object_id, document_data)?;

        let parent_path = file_path.parent().unwrap();
        self.replace_link_views(&mut document, external_link_views);
        self
          .replace_resources(
            &self.workspace_id,
            &mut document,
            &mut resource_paths,
            parent_path,
          )
          .await;

        let resources = resource_paths
          .iter()
          .filter_map(|p| p.to_str().map(|s| s.to_string()))
          .collect();

        Ok((document, resources))
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
    resources: &mut Vec<PathBuf>,
    parent_path: &Path,
  ) {
    let mut document_resources = HashSet::new();
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
              let pos = resources.iter().position(|r| r == &full_image_url);
              if let Some(pos) = pos {
                if let Ok(file_id) = FileId::from_path(&full_image_url).await {
                  document_resources.insert(resources.remove(pos));

                  let url = upload_file_url(&self.host, workspace_id, &self.object_id, &file_id);
                  block_data.insert(URL_FIELD.to_string(), json!(url));
                  // Update the block with the new URL
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

    *resources = document_resources.into_iter().collect();
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

  pub async fn as_database(&self) -> Result<(Database, Vec<String>), ImporterError> {
    match &self.notion_file {
      NotionFile::CSV { file_path, .. } => {
        let content = std::fs::read_to_string(file_path)?;
        let resources = self
          .notion_file
          .upload_resources()
          .iter()
          .filter_map(|p| p.to_str().map(|s| s.to_string()))
          .collect();
        let csv_template = CSVTemplate::try_from_reader_with_resources(
          Some(self.host.clone()),
          self.workspace_id.clone(),
          content.as_bytes(),
          true,
          resources,
        )?;
        let resources = csv_template.resources.clone();
        let database_view_id = gen_database_view_id();
        let database_template = csv_template.try_into_database_template().await.unwrap();
        let database =
          Database::create_with_template(&self.object_id, &database_view_id, database_template)
            .await?;
        Ok((database, resources))
      },
      _ => Err(ImporterError::InvalidFileType(format!(
        "File type is not supported for database: {:?}",
        self.notion_file
      ))),
    }
  }

  #[async_recursion(?Send)]
  pub async fn build_imported_collab_recursively(&self) -> Vec<ImportedCollabInfo> {
    let mut imported_collab_info_list = vec![];
    let imported_collab_info = self.build_imported_collab().await;
    if let Ok(info) = imported_collab_info {
      imported_collab_info_list.push(info);
    }
    for child in self.children.iter() {
      imported_collab_info_list.extend(child.build_imported_collab_recursively().await);
    }
    imported_collab_info_list
  }

  pub async fn build_imported_collab(&self) -> Result<ImportedCollabInfo, ImporterError> {
    let name = self.notion_name.clone();
    match &self.notion_file {
      NotionFile::CSV { .. } => {
        let (database, files) = self.as_database().await?;
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

        let file_size = calculate_files_size(&files);

        Ok(ImportedCollabInfo {
          name,
          collabs: imported_collabs,
          files,
          file_size,
          import_type: ImportType::Database,
        })
      },
      NotionFile::Markdown { .. } => {
        let (document, files) = self.as_document(HashMap::new()).await?;
        let encoded_collab = document.encode_collab()?;
        let imported_collab = ImportedCollab {
          object_id: self.object_id.clone(),
          collab_type: CollabType::Document,
          encoded_collab,
        };
        let file_size = calculate_files_size(&files);
        Ok(ImportedCollabInfo {
          name,
          collabs: vec![imported_collab],
          files,
          file_size,
          import_type: ImportType::Document,
        })
      },
      _ => Err(ImporterError::InvalidFileType(format!(
        "File type is not supported for collab: {:?}",
        self.notion_file
      ))),
    }
  }
}

fn calculate_files_size(paths: &[String]) -> u64 {
  paths
    .iter()
    .map(|p| std::fs::metadata(p).map(|m| m.len()).unwrap_or(0))
    .sum()
}

#[derive(Debug, Clone, Serialize)]
pub struct ExternalLink {
  pub id: String,
  pub name: String,
  pub link_type: ExternalLinkType,
}

#[derive(Debug, Clone, Serialize)]
pub enum ExternalLinkType {
  Unknown,
  CSV,
  Markdown,
}
