use crate::error::ImporterError;
use crate::imported_collab::{ImportType, ImportedCollab, ImportedCollabInfo};

use collab_database::database::{Database, get_row_document_id};
use collab_database::template::csv::{CSVResource, CSVTemplate};
use collab_document::blocks::{BlockType, TextDelta, mention_block_data, mention_block_delta};
use collab_document::document::Document;
use collab_document::importer::define::URL_FIELD;
use collab_document::importer::md_importer::{MDImporter, create_image_block};
use collab_entity::CollabType;
use futures::stream::{self, StreamExt};

use crate::notion::file::NotionFile;
use crate::notion::walk_dir::{extract_delta_link, extract_external_links};
use crate::notion::{CSVRelation, ImportedCollabInfoStream};
use crate::util::{FileId, upload_file_url};
use collab::core::collab::default_client_id;
use collab_database::database_trait::NoPersistenceDatabaseCollabService;
use collab_database::rows::RowId;
use collab_database::template::builder::FileUrlBuilder;
use collab_document::document_data::default_document_data;
use percent_encoding::percent_decode_str;
use serde::Serialize;
use serde_json::json;
use std::collections::{HashMap, HashSet};
use std::fmt::Display;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::fs;
use tracing::error;

#[derive(Debug, Clone)]
pub struct NotionPage {
  pub notion_name: String,
  pub notion_id: Option<String>,
  pub notion_file: NotionFile,
  /// If current notion view is database, then view_id is the inline view id of the database.
  /// If current notion view is document, then view_id is the document id of the document.
  pub view_id: String,
  pub workspace_id: String,
  pub children: Vec<NotionPage>,
  pub external_links: Vec<Vec<ExternalLink>>,
  pub host: String,
  pub is_dir: bool,
  pub csv_relation: CSVRelation,
}

impl NotionPage {
  pub fn turn_into_space(&mut self) {
    self.is_dir = true;
    self.children.clear();
    self.notion_file = NotionFile::Empty;
    self.external_links.clear();
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

  pub fn get_external_link_notion_view(&self) -> HashMap<String, NotionPage> {
    let mut linked_views = HashMap::new();
    for links in self.external_links.iter() {
      if let Some(link) = links.last() {
        let page = self.csv_relation.get_page(&link.file_name);
        if let Some(page) = page {
          linked_views.insert(link.id.clone(), page);
        } else if let Some(view) = self.get_view(&link.id) {
          linked_views.insert(link.id.clone(), view);
        }
      }
    }
    linked_views
  }

  pub fn get_external_linked_views(&self) -> Vec<NotionPage> {
    self.get_external_link_notion_view().into_values().collect()
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

  /// Get the view with the given ID.
  /// It will search the view recursively.
  pub fn get_view(&self, id: &str) -> Option<NotionPage> {
    fn search_view(views: &[NotionPage], id: &str) -> Option<NotionPage> {
      for view in views {
        if let Some(notion_id) = &view.notion_id {
          if notion_id == id {
            return Some(view.clone());
          }
        }
        if let Some(child_view) = search_view(&view.children, id) {
          return Some(child_view);
        }
      }
      None
    }

    search_view(&self.children, id)
  }

  pub async fn as_document(&self) -> Result<(Document, CollabResource), ImporterError> {
    let external_link_views = self.get_external_link_notion_view();
    match &self.notion_file {
      NotionFile::Markdown { file_path, .. } => {
        let resource_paths = self.notion_file.upload_files();
        let md_importer = MDImporter::new(None);
        let content = fs::read_to_string(file_path).await?;
        let document_data = md_importer.import(&self.view_id, content)?;
        let mut document = Document::create(&self.view_id, document_data, default_client_id())?;

        let url_builder = |view_id, path| async move {
          let file_id = FileId::from_path(&path).await.ok()?;
          Some(upload_file_url(
            &self.host,
            &self.workspace_id,
            view_id,
            &file_id,
          ))
        };
        let parent_path = file_path.parent().unwrap();
        let valid_delta_resources = self
          .replace_link_views(
            parent_path,
            &mut document,
            &resource_paths,
            external_link_views,
            &url_builder,
          )
          .await;
        let valid_block_resources = self
          .replace_block_resources_recursively(
            parent_path,
            &mut document,
            &resource_paths,
            url_builder,
          )
          .await;

        let all_resources = valid_block_resources
          .into_iter()
          .chain(valid_delta_resources.into_iter())
          .collect::<Vec<_>>();

        let files = all_resources
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

  async fn replace_block_resources_recursively<'a, B, O>(
    &'a self,
    parent_path: &Path,
    document: &mut Document,
    resources: &[PathBuf],
    file_url_builder: B,
  ) -> Vec<PathBuf>
  where
    B: Fn(&'a str, PathBuf) -> O + Send + Sync + 'a,
    O: Future<Output = Option<String>> + Send + 'a,
  {
    let mut document_resources = HashSet::new();
    if let Some(page_id) = document.get_page_id() {
      // Start the recursive processing with the root block (page_id)
      self
        .process_block_and_children(
          parent_path,
          document,
          &page_id,
          resources,
          &file_url_builder,
          &mut document_resources,
        )
        .await;
    }

    document_resources.into_iter().collect()
  }

  #[async_recursion::async_recursion(?Send)]
  async fn process_block_and_children<'a, B, O>(
    &'a self,
    parent_path: &Path,
    document: &mut Document,
    block_id: &str,
    resources: &[PathBuf],
    file_url_builder: &B,
    document_resources: &mut HashSet<PathBuf>,
  ) where
    B: Fn(&'a str, PathBuf) -> O + Send + Sync + 'a,
    O: Future<Output = Option<String>> + Send + 'a,
  {
    // Process the current block
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
              document_resources.insert(resources[pos].clone());
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

    // Recursively process each child block
    let block_children_ids = document.get_block_children_ids(block_id);
    for child_id in block_children_ids.iter() {
      self
        .process_block_and_children(
          parent_path,
          document,
          child_id,
          resources,
          file_url_builder,
          document_resources,
        )
        .await;
    }
  }

  async fn replace_link_views<'a, 'b, B, O>(
    &'b self,
    parent_path: &Path,
    document: &mut Document,
    resources: &[PathBuf],
    external_link_views: HashMap<String, NotionPage>,
    file_url_builder: &'a B,
  ) -> Vec<PathBuf>
  where
    B: Fn(&'a str, PathBuf) -> O + Send + Sync + 'a,
    O: Future<Output = Option<String>> + Send + 'a,
    'b: 'a,
  {
    let mut delta_resources = HashSet::new();
    if let Some(first_page_id) = document.get_page_id() {
      // Start the recursive processing with the first page's root block
      self
        .process_link_views_recursive(
          parent_path,
          document,
          &first_page_id,
          resources,
          &external_link_views,
          file_url_builder,
          &mut delta_resources,
        )
        .await;
    }

    delta_resources.into_iter().collect()
  }

  #[async_recursion::async_recursion]
  #[allow(clippy::too_many_arguments)]
  async fn process_link_views_recursive<'a, 'b, B, O>(
    &'b self,
    parent_path: &Path,
    document: &mut Document,
    block_id: &str,
    resources: &[PathBuf],
    external_link_views: &HashMap<String, NotionPage>,
    file_url_builder: &'a B,
    delta_resources: &mut HashSet<PathBuf>,
  ) where
    B: Fn(&'a str, PathBuf) -> O + Send + Sync + 'a,
    O: Future<Output = Option<String>> + Send + 'a,
    'b: 'a,
  {
    if let Some((block_type, deltas)) = document.get_block_delta(block_id) {
      let block_deltas_result = self
        .process_block_deltas(
          parent_path,
          document,
          block_id,
          block_type,
          deltas,
          external_link_views,
          resources,
          file_url_builder,
        )
        .await;

      // Collect resources from this block
      delta_resources.extend(block_deltas_result.delta_resources);

      // Update document deltas if new ones are created
      if let Some(new_deltas) = block_deltas_result.new_deltas {
        if let Err(err) = document.set_block_delta(block_id, new_deltas) {
          error!(
            "Failed to set block delta when trying to replace ref link. error: {:?}",
            err
          );
        }
      }

      // Insert new image blocks if any are created
      for image_url in block_deltas_result.new_delta_image_blocks {
        let new_block_id = collab_document::document_data::generate_id();
        let image_block = create_image_block(&new_block_id, image_url, block_id);
        if let Err(err) = document.insert_block(image_block, Some(block_id.to_string())) {
          error!(
            "Failed to insert image block when trying to replace delta link. error: {:?}",
            err
          );
        }
      }
    }

    // Recursively process each child block
    let block_children_ids = document.get_block_children_ids(block_id);
    for child_id in block_children_ids.iter() {
      self
        .process_link_views_recursive(
          parent_path,
          document,
          child_id,
          resources,
          external_link_views,
          file_url_builder,
          delta_resources,
        )
        .await;
    }
  }

  /// Process the deltas for a block, looking for links to replace
  #[allow(clippy::too_many_arguments)]
  async fn process_block_deltas<'a, 'b, B, O>(
    &'b self,
    parent_path: &Path,
    document: &mut Document,
    block_id: &str,
    block_type: BlockType,
    mut deltas: Vec<TextDelta>,
    external_link_views: &HashMap<String, NotionPage>,
    resources: &[PathBuf],
    file_url_builder: &'a B,
  ) -> ProcessBlockDeltaResult
  where
    B: Fn(&'a str, PathBuf) -> O + Send + Sync + 'a,
    O: Future<Output = Option<String>> + Send + 'a,
    'b: 'a,
  {
    let mut is_changed = false;
    let mut new_delta_image_blocks = vec![];
    let mut delta_resources = HashSet::new();
    for delta in deltas.iter_mut() {
      if let TextDelta::Inserted(v, attrs) = delta.clone() {
        // If there are any href in the attrs, we will try to replace with the corresponding view id
        if let Some(attrs) = &attrs {
          if let Some(href_value) = attrs.get("href") {
            let delta_str = href_value.to_string();

            // Replace links in the deltas with the corresponding view IDs
            if let Ok(links) = extract_external_links(&delta_str) {
              for link in &links {
                if let Some(view) = external_link_views.get(&link.id) {
                  if v == link.name {
                    is_changed = true;
                    *delta = mention_block_delta(&view.view_id);
                  }
                }
              }

              self.update_paragraph_block(
                document,
                block_id,
                &block_type,
                &links,
                external_link_views,
              );
            }
          }
        }

        // extract link from insert delta and replace with real image link
        if let Some(delta_link) = extract_delta_link(&v) {
          debug_assert!(attrs.is_none(), "attrs should be None for image link");
          let full_image_url = parent_path.join(delta_link.link);
          let pos = resources.iter().position(|r| r == &full_image_url);
          if let Some(pos) = pos {
            if let Some(url) = file_url_builder(&self.view_id, full_image_url).await {
              delta_resources.insert(resources[pos].clone());

              // replace the inserted image link with empty string
              *delta = TextDelta::Inserted("".to_string(), None);
              // generate a block for given image
              new_delta_image_blocks.push(url);
            }
          }
        }
      }
    }

    let mut result = ProcessBlockDeltaResult {
      delta_resources: delta_resources.into_iter().collect(),
      new_deltas: None,
      new_delta_image_blocks,
    };

    if is_changed {
      result.new_deltas = Some(deltas);
    }

    result
  }

  /// Update the paragraph block if the last link points to an external view
  fn update_paragraph_block(
    &self,
    document: &mut Document,
    block_id: &str,
    block_type: &BlockType,
    links: &[ExternalLink],
    external_link_views: &HashMap<String, NotionPage>,
  ) {
    if let Some(last_link) = links.last() {
      if let Some(view) = external_link_views.get(&last_link.id) {
        if matches!(block_type, BlockType::Paragraph) {
          let data = mention_block_data(&view.view_id, &self.view_id);
          if let Err(err) = document.update_block(block_id, data) {
            error!(
              "Failed to update block when trying to replace ref link. error: {:?}",
              err
            );
          }
        }
      }
    }
  }

  pub async fn as_database(&self) -> Result<DatabaseImportContent, ImporterError> {
    match &self.notion_file {
      NotionFile::CSV {
        file_path,
        row_documents,
        ..
      } => {
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
          .await?;
        let service = Arc::new(NoPersistenceDatabaseCollabService::new(default_client_id()));
        let mut database =
          Database::create_with_template(database_template, service.clone(), service).await?;
        let mut row_documents = row_documents.clone();

        if let Some(field) = database.get_primary_field() {
          let view_id = database.get_first_database_view_id().unwrap();
          let row_cells = database
            .get_cells_for_field(&view_id, &field.id, false)
            .await;
          for row_cell in row_cells {
            for row_document in row_documents.iter_mut() {
              if let Some(text) = row_cell.text() {
                if row_document.page.notion_name == text {
                  row_document.set_row_document_id(&row_cell.row_id);
                  database
                    .update_row_meta(&row_cell.row_id, |meta| {
                      meta.update_is_document_empty(false);
                    })
                    .await;
                }
              }
            }
          }
        }

        let resource = CollabResource {
          object_id: database_id,
          files,
        };

        Ok(DatabaseImportContent {
          database,
          row_documents,
          resource,
        })
      },
      _ => Err(ImporterError::InvalidFileType(format!(
        "File type is not supported for database: {:?}",
        self.notion_file
      ))),
    }
  }

  #[async_recursion::async_recursion(?Send)]
  pub async fn build_imported_collab(&self) -> Result<Option<ImportedCollabInfo>, ImporterError> {
    let name = self.notion_name.clone();
    match &self.notion_file {
      NotionFile::CSV { .. } => {
        let content = self.as_database().await?;
        let database_id = content.database.get_database_id();
        let mut resources = vec![content.resource];
        let view_ids = content
          .database
          .get_all_views()
          .into_iter()
          .map(|view| view.id)
          .collect::<Vec<_>>();

        let mut imported_collabs = content
          .database
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

        let mut row_document_ids = vec![];
        for row_document in content.row_documents {
          if let Ok((document, resource)) = row_document.page.as_document().await {
            if let Ok(encoded_collab) = document.encode_collab() {
              resources.push(resource);
              let imported_collab = ImportedCollab {
                object_id: row_document.page.view_id.clone(),
                collab_type: CollabType::Document,
                encoded_collab,
              };
              imported_collabs.push(imported_collab);
              row_document_ids.push(row_document.page.view_id.clone())
            }
          }

          for child in row_document.page.children {
            if let Ok(Some(value)) = child.build_imported_collab().await {
              imported_collabs.extend(value.imported_collabs);
              resources.extend(value.resources);
            }
          }
        }

        Ok(Some(ImportedCollabInfo {
          name,
          imported_collabs,
          resources,
          import_type: ImportType::Database {
            database_id,
            view_ids,
            row_document_ids,
          },
        }))
      },
      NotionFile::Markdown { .. } => {
        let (document, collab_resource) = self.as_document().await?;
        let encoded_collab = document.encode_collab()?;
        let imported_collab = ImportedCollab {
          object_id: self.view_id.clone(),
          collab_type: CollabType::Document,
          encoded_collab,
        };
        Ok(Some(ImportedCollabInfo {
          name,
          imported_collabs: vec![imported_collab],
          resources: vec![collab_resource],
          import_type: ImportType::Document,
        }))
      },
      NotionFile::Empty => {
        let data = default_document_data(&self.view_id);
        let document = Document::create(&self.view_id, data, default_client_id())?;
        let encoded_collab = document.encode_collab()?;
        let imported_collab = ImportedCollab {
          object_id: self.view_id.clone(),
          collab_type: CollabType::Document,
          encoded_collab,
        };
        Ok(Some(ImportedCollabInfo {
          name,
          imported_collabs: vec![imported_collab],
          resources: vec![CollabResource {
            object_id: self.view_id.clone(),
            files: vec![],
          }],
          import_type: ImportType::Document,
        }))
      },
      _ => Ok(None),
    }
  }
}

pub async fn build_imported_collab_recursively<'a>(
  notion_page: NotionPage,
) -> ImportedCollabInfoStream<'a> {
  let imported_collab_info = notion_page.build_imported_collab().await;
  let initial_stream: ImportedCollabInfoStream = match imported_collab_info {
    Ok(Some(info)) => Box::pin(stream::once(async { info })),
    Ok(None) => Box::pin(stream::empty()),
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

pub struct ProcessBlockDeltaResult {
  pub delta_resources: Vec<PathBuf>,
  pub new_deltas: Option<Vec<TextDelta>>,
  pub new_delta_image_blocks: Vec<String>,
}

#[derive(Debug, Default, Clone, Eq, PartialEq, Serialize)]
pub struct ExternalLink {
  pub id: String,
  pub name: String,
  pub link_type: ExternalLinkType,
  pub file_name: String,
}

#[derive(Debug, Default, Clone, Eq, PartialEq, Serialize)]
pub enum ExternalLinkType {
  #[default]
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

#[derive(Debug, Clone)]
pub struct ImportedRowDocument {
  pub page: NotionPage,
}

impl ImportedRowDocument {
  fn set_row_document_id(&mut self, row_id: &RowId) {
    let document_id = get_row_document_id(row_id).unwrap();
    self.page.view_id = document_id;
  }
}

impl Display for ImportedRowDocument {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}", self.page.notion_name)
  }
}

pub struct DatabaseImportContent {
  pub database: Database,
  pub row_documents: Vec<ImportedRowDocument>,
  pub resource: CollabResource,
}
