use crate::error::ImporterError;
use crate::imported_collab::{ImportType, ImportedCollab, ImportedCollabInfo};
use crate::notion::file::NotionFile;
use crate::notion::page::{build_imported_collab_recursively, CollabResource, NotionPage};
use crate::notion::walk_dir::{file_name_from_path, process_entry};
use crate::space_view::create_space_view;
use collab::preclude::Collab;

use collab_folder::hierarchy_builder::{
  NestedChildViewBuilder, NestedViews, ParentChildViews, SpacePermission,
};
use collab_folder::ViewLayout;
use futures::stream;
use futures::stream::{Stream, StreamExt};

use collab_entity::CollabType;
use std::path::PathBuf;
use std::pin::Pin;
use walkdir::WalkDir;

#[derive(Debug)]
pub struct NotionImporter {
  uid: i64,
  host: String,
  workspace_id: String,
  path: PathBuf,
  name: String,
  pub views: Option<NotionPage>,
}

impl NotionImporter {
  pub fn new<P: Into<PathBuf>, S: ToString>(
    uid: i64,
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
      uid,
      host,
      workspace_id: workspace_id.to_string(),
      path,
      name,
      views: None,
    })
  }

  /// Return a ImportedInfo struct that contains all the views and their children recursively.
  pub async fn import(mut self) -> Result<ImportedInfo, ImporterError> {
    let views = self.collect_views().await?;
    ImportedInfo::new(
      self.uid,
      self.workspace_id.clone(),
      self.host.clone(),
      self.name.clone(),
      views,
    )
  }

  async fn collect_views(&mut self) -> Result<Vec<NotionPage>, ImporterError> {
    let views = WalkDir::new(&self.path)
      .max_depth(1)
      .into_iter()
      .filter_map(|e| e.ok())
      .filter_map(|entry| process_entry(&self.host, &self.workspace_id, &entry))
      .collect::<Vec<NotionPage>>();
    Ok(views)
  }
}

#[derive(Debug)]
pub struct ImportedInfo {
  pub uid: i64,
  pub workspace_id: String,
  pub host: String,
  pub name: String,
  views: Vec<NotionPage>,
  space_view: ParentChildViews,
  space_collab: Collab,
}

pub type ImportedCollabInfoStream<'a> = Pin<Box<dyn Stream<Item = ImportedCollabInfo> + 'a>>;
impl ImportedInfo {
  pub fn new(
    uid: i64,
    workspace_id: String,
    host: String,
    name: String,
    views: Vec<NotionPage>,
  ) -> Result<Self, ImporterError> {
    let view_id = uuid::Uuid::new_v4().to_string();
    let (space_view, space_collab) = create_space_view(
      uid,
      &workspace_id,
      &name,
      &view_id,
      vec![],
      SpacePermission::PublicToAll,
    )?;
    Ok(Self {
      uid,
      workspace_id,
      host,
      name,
      views,
      space_view,
      space_collab,
    })
  }

  pub fn views(&self) -> &Vec<NotionPage> {
    &self.views
  }

  pub async fn into_collab_stream(self) -> ImportedCollabInfoStream<'static> {
    // Create a stream for each view by resolving the futures into streams
    let view_streams = self
      .views
      .into_iter()
      .map(|view| async { build_imported_collab_recursively(view).await });

    let imported_space_collab = ImportedCollab {
      object_id: self.space_view.view.id.clone(),
      collab_type: CollabType::Document,
      encoded_collab: self
        .space_collab
        .encode_collab_v1(|_collab| Ok::<_, ImporterError>(()))
        .unwrap(),
    };

    let space_view_collab = ImportedCollabInfo {
      name: self.name.clone(),
      collabs: vec![imported_space_collab],
      resource: CollabResource {
        object_id: self.space_view.view.id,
        files: vec![],
      },
      import_type: ImportType::Document,
    };
    let space_view_collab_stream = stream::once(async { space_view_collab });
    let combined_view_stream = stream::iter(view_streams)
      .then(|stream_future| stream_future)
      .flatten();
    let combined_stream = space_view_collab_stream.chain(combined_view_stream);
    Box::pin(combined_stream) as ImportedCollabInfoStream
  }

  pub async fn build_nested_views(&self) -> NestedViews {
    let mut space_view = self.space_view.clone();
    let parent_id = space_view.view.id.clone();
    let views: Vec<ParentChildViews> = stream::iter(&self.views)
      .then(|notion_page| convert_notion_page_to_parent_child(&parent_id, notion_page, self.uid))
      .collect()
      .await;

    space_view.children = views;
    NestedViews {
      views: vec![space_view],
    }
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

#[async_recursion::async_recursion]
async fn convert_notion_page_to_parent_child(
  parent_id: &str,
  notion_page: &NotionPage,
  uid: i64,
) -> ParentChildViews {
  let view_layout = match notion_page.notion_file {
    NotionFile::Unknown => ViewLayout::Document,
    NotionFile::CSV { .. } => ViewLayout::Grid,
    NotionFile::CSVPart { .. } => ViewLayout::Grid,
    NotionFile::Markdown { .. } => ViewLayout::Document,
  };
  let mut view_builder = NestedChildViewBuilder::new(uid, parent_id.to_string())
    .with_name(&notion_page.notion_name)
    .with_layout(view_layout)
    .with_view_id(&notion_page.view_id);

  for child_notion_page in &notion_page.children {
    view_builder = view_builder
      .with_child_view_builder(|_| async {
        convert_notion_page_to_parent_child(&notion_page.view_id, child_notion_page, uid).await
      })
      .await;
  }

  view_builder.build()
}
