use crate::error::ImporterError;
use crate::imported_collab::{ImportType, ImportedCollab, ImportedCollabInfo};

use collab_database::database::Database;
use collab_database::template::csv::{CSVResource, CSVTemplate};
use collab_document::blocks::{mention_block_data, mention_block_delta, TextDelta};
use collab_document::document::Document;
use collab_document::importer::define::{BlockType, URL_FIELD};
use collab_document::importer::md_importer::MDImporter;
use collab_entity::CollabType;
use futures::stream::{self, StreamExt};

use crate::notion::file::NotionFile;
use crate::notion::walk_dir::extract_external_links;
use crate::notion::ImportedCollabInfoStream;
use crate::util::{upload_file_url, FileId};
use collab_database::template::builder::FileUrlBuilder;
use percent_encoding::percent_decode_str;
use serde::Serialize;
use serde_json::json;
use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::path::{Path, PathBuf};
use tokio::fs;
use tracing::error;

#[derive(Debug, Clone, Serialize)]
pub struct NotionPage {
  pub notion_name: String,
  pub notion_id: String,
  pub notion_file: NotionFile,
  /// If current notion view is database, then view_id is the inline view id of the database.
  /// If current notion view is document, then view_id is the document id of the document.
  pub view_id: String,
  pub workspace_id: String,
  pub children: Vec<NotionPage>,
  pub external_links: Vec<Vec<ExternalLink>>,
  pub host: String,
}

impl NotionPage {
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

  pub fn get_external_link_notion_view(&self) -> HashMap<String, NotionPage> {
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
  pub fn get_view(&self, id: &str) -> Option<NotionPage> {
    fn search_view(views: &[NotionPage], id: &str) -> Option<NotionPage> {
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

  pub fn get_linked_views(&self) -> Vec<NotionPage> {
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
    external_link_views: HashMap<String, NotionPage>,
  ) -> Result<(Document, CollabResource), ImporterError> {
    match &self.notion_file {
      NotionFile::Markdown { file_path, .. } => {
        let mut file_paths = self.notion_file.upload_files();
        let md_importer = MDImporter::new(None);
        let content = fs::read_to_string(file_path).await?;
        let document_data = md_importer.import(&self.view_id, content)?;
        let mut document = Document::create(&self.view_id, document_data)?;

        let parent_path = file_path.parent().unwrap();
        self.replace_link_views(&mut document, external_link_views);
        self
          .replace_resources(
            &mut document,
            &mut file_paths,
            parent_path,
            |view_id, path| async move {
              let file_id = FileId::from_path(&path).await.ok()?;
              Some(upload_file_url(
                &self.host,
                &self.workspace_id,
                view_id,
                &file_id,
              ))
            },
          )
          .await;

        let files = file_paths
          .iter()
          .filter_map(|p| p.to_str().map(|s| s.to_string()))
          .collect();

        let resource = CollabResource {
          object_id: self.view_id.clone(),
          files,
        };

        Ok((document, resource))
      },
      _ => Err(ImporterError::InvalidFileType(format!(
        "File type is not supported for document: {:?}",
        self.notion_file
      ))),
    }
  }

  async fn replace_resources<'a, B, O>(
    &'a self,
    document: &mut Document,
    resources: &mut Vec<PathBuf>,
    parent_path: &Path,
    file_url_builder: B,
  ) where
    B: Fn(&'a str, PathBuf) -> O + Send + Sync + 'a,
    O: Future<Output = Option<String>> + Send + 'a,
  {
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
                if let Some(url) = file_url_builder(&self.view_id, full_image_url).await {
                  document_resources.insert(resources.remove(pos));
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

    *resources = document_resources.into_iter().collect();
  }

  fn replace_link_views(
    &self,
    document: &mut Document,
    external_link_views: HashMap<String, NotionPage>,
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
                        let data = mention_block_data(&view.view_id, &self.view_id);
                        if let Err(err) = document.update_block(block_id, data) {
                          error!(
                            "Failed to update block when trying to replace ref link. error:{:?}",
                            err
                          );
                        }
                      } else {
                        let delta = mention_block_delta(&view.view_id);
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

  pub async fn as_database(&self) -> Result<(Database, CollabResource), ImporterError> {
    match &self.notion_file {
      NotionFile::CSV { file_path, .. } => {
        let content = fs::read_to_string(file_path).await?;
        let files = self
          .notion_file
          .upload_files()
          .iter()
          .filter_map(|p| p.to_str().map(|s| s.to_string()))
          .collect();

        let csv_resource = CSVResource {
          server_url: self.host.clone(),
          workspace_id: self.workspace_id.clone(),
          files,
        };

        // create csv template, we need to set the view id as csv template view id
        let mut csv_template =
          CSVTemplate::try_from_reader(content.as_bytes(), true, Some(csv_resource))?;
        csv_template.reset_view_id(self.view_id.clone());
        let database_id = csv_template.database_id.clone();

        let file_url_builder = FileUrlBuilderImpl {
          host: self.host.clone(),
          workspace_id: self.workspace_id.clone(),
        };

        let files = csv_template.resource.as_ref().unwrap().files.clone();
        let database_template = csv_template
          .try_into_database_template(Some(Box::new(file_url_builder)))
          .await
          .unwrap();
        let database = Database::create_with_template(database_template).await?;
        let resource = CollabResource {
          object_id: database_id,
          files,
        };
        Ok((database, resource))
      },
      _ => Err(ImporterError::InvalidFileType(format!(
        "File type is not supported for database: {:?}",
        self.notion_file
      ))),
    }
  }

  pub async fn build_imported_collab(&self) -> Result<ImportedCollabInfo, ImporterError> {
    let name = self.notion_name.clone();
    match &self.notion_file {
      NotionFile::CSV { .. } => {
        let (database, collab_resource) = self.as_database().await?;
        let database_id = database.get_database_id();
        let view_ids = database
          .get_all_views()
          .into_iter()
          .map(|view| view.id)
          .collect::<Vec<_>>();
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

        Ok(ImportedCollabInfo {
          name,
          collabs: imported_collabs,
          resource: collab_resource,
          import_type: ImportType::Database {
            database_id,
            view_ids,
          },
        })
      },
      NotionFile::Markdown { .. } => {
        let (document, collab_resource) = self.as_document(HashMap::new()).await?;
        let encoded_collab = document.encode_collab()?;
        let imported_collab = ImportedCollab {
          object_id: self.view_id.clone(),
          collab_type: CollabType::Document,
          encoded_collab,
        };
        Ok(ImportedCollabInfo {
          name,
          collabs: vec![imported_collab],
          resource: collab_resource,
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

pub async fn build_imported_collab_recursively<'a>(
  notion_page: NotionPage,
) -> ImportedCollabInfoStream<'a> {
  let imported_collab_info = notion_page.build_imported_collab().await;
  let initial_stream: ImportedCollabInfoStream = match imported_collab_info {
    Ok(info) => Box::pin(stream::once(async { info })),
    Err(_) => Box::pin(stream::empty()),
  };

  let child_streams = notion_page
    .children
    .into_iter()
    .map(|child| async move { build_imported_collab_recursively(child).await });

  let child_stream = stream::iter(child_streams)
    .then(|stream_future| stream_future)
    .flatten();

  Box::pin(initial_stream.chain(child_stream))
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

#[derive(Debug, Clone)]
pub struct CollabResource {
  pub object_id: String,
  pub files: Vec<String>,
}

struct FileUrlBuilderImpl {
  host: String,
  workspace_id: String,
}

#[async_trait::async_trait]
impl FileUrlBuilder for FileUrlBuilderImpl {
  async fn build(&self, database_id: &str, path: &Path) -> Option<String> {
    let file_id = FileId::from_path(&path.to_path_buf()).await.ok()?;
    Some(upload_file_url(
      &self.host,
      &self.workspace_id,
      database_id,
      &file_id,
    ))
  }
}
