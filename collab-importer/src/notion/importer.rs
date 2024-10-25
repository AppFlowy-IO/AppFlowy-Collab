use crate::error::ImporterError;
use crate::imported_collab::{ImportType, ImportedCollab, ImportedCollabInfo};
use crate::notion::file::NotionFile;
use crate::notion::page::{build_imported_collab_recursively, CollabResource, NotionPage};
use crate::notion::walk_dir::{file_name_from_path, process_entry, walk_sub_dir};
use collab_folder::hierarchy_builder::{
  NestedChildViewBuilder, NestedViews, ParentChildViews, SpacePermission, ViewExtraBuilder,
};
use collab_folder::ViewLayout;
use futures::stream;
use futures::stream::{Stream, StreamExt};
use std::collections::{HashMap, HashSet};
use std::ops::Deref;

use crate::space_view::create_space_view;
use anyhow::Error;
use collab::preclude::Collab;
use collab_entity::CollabType;
use csv::Reader;
use fancy_regex::Regex;
use std::path::PathBuf;
use std::pin::Pin;
use walkdir::WalkDir;

#[derive(Debug)]
pub struct NotionImporter {
  uid: i64,
  host: String,
  workspace_id: String,
  path: PathBuf,
  workspace_name: String,
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
      return Err(ImporterError::InvalidPath(format!(
        "Path: does not exist: {:?}",
        path
      )));
    }

    let workspace_name = file_name_from_path(&path).unwrap_or_else(|_| {
      let now = chrono::Utc::now();
      format!("import-{}", now.format("%Y-%m-%d %H:%M"))
    });

    Ok(Self {
      uid,
      host,
      workspace_id: workspace_id.to_string(),
      path,
      workspace_name,
      views: None,
    })
  }

  /// Return a ImportedInfo struct that contains all the views and their children recursively.
  pub async fn import(mut self) -> Result<ImportedInfo, ImporterError> {
    let views = self.collect_pages().await?;
    if views.is_empty() {
      return Err(ImporterError::CannotImport);
    }

    ImportedInfo::new(
      self.uid,
      self.workspace_id.clone(),
      self.host.clone(),
      self.workspace_name.clone(),
      views,
    )
  }

  async fn collect_pages(&mut self) -> Result<Vec<NotionPage>, ImporterError> {
    let mut notion_pages = vec![];
    let mut has_spaces = false;
    let mut has_pages = false;

    let csv_relation = find_parent_child_csv_relationships(&self.path).unwrap_or_default();

    // Process entries and track whether we have spaces (directories) and pages (non-directories)
    for entry in walk_sub_dir(&self.path) {
      if let Some(view) =
        process_entry(&self.host, &self.workspace_id, &entry, false, &csv_relation)
      {
        has_spaces |= view.is_dir;
        has_pages |= !view.is_dir;
        notion_pages.push(view);
      }
    }

    // If there are only spaces (directories) and no pages, return the pages
    if !has_pages && has_spaces {
      return Ok(notion_pages);
    }

    if has_pages && has_spaces {
      notion_pages.iter_mut().for_each(|page| {
        if !page.is_dir {
          let mut cloned_page = page.clone();
          cloned_page.is_dir = false;

          page.turn_into_space();
          page.children.push(cloned_page);
        }
      });
    }

    Ok(notion_pages)
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
      "Imported Space",
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

  fn has_space_view(&self) -> bool {
    !self.views.iter().any(|view| !view.is_dir)
  }

  fn space_ids(&self) -> Vec<String> {
    let mut space_ids = Vec::new();
    for view in &self.views {
      if view.is_dir {
        space_ids.push(view.view_id.clone());
      }
    }
    space_ids
  }

  pub async fn into_collab_stream(self) -> ImportedCollabInfoStream<'static> {
    // Create a stream for each view by resolving the futures into streams
    let has_space = self.has_space_view();
    let view_streams = self
      .views
      .into_iter()
      .map(|view| async { build_imported_collab_recursively(view).await });

    if has_space {
      let combined_stream = stream::iter(view_streams)
        .then(|stream_future| stream_future)
        .flatten();
      Box::pin(combined_stream) as ImportedCollabInfoStream
    } else {
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
        imported_collabs: vec![imported_space_collab],
        resources: vec![CollabResource {
          object_id: self.space_view.view.id,
          files: vec![],
        }],
        import_type: ImportType::Document,
      };

      let space_view_collab_stream = stream::once(async { space_view_collab });
      let combined_view_stream = stream::iter(view_streams)
        .then(|stream_future| stream_future)
        .flatten();
      let combined_stream = space_view_collab_stream.chain(combined_view_stream);
      Box::pin(combined_stream) as ImportedCollabInfoStream
    }
  }

  pub async fn build_nested_views(&self) -> NestedViews {
    let space_ids = self.space_ids();
    let parent_id = if space_ids.is_empty() {
      self.space_view.view.id.clone()
    } else {
      self.workspace_id.clone()
    };

    let mut views: Vec<ParentChildViews> = stream::iter(&self.views)
      .then(|notion_page| convert_notion_page_to_parent_child(&parent_id, notion_page, self.uid))
      .collect()
      .await;

    let views = if space_ids.is_empty() {
      let mut space_view = self.space_view.clone();
      space_view.children = views;
      vec![space_view]
    } else {
      views.iter_mut().for_each(|view| {
        if space_ids.contains(&view.view.id) {
          view.view.extra = serde_json::to_string(
            &ViewExtraBuilder::new()
              .is_space(true, SpacePermission::PublicToAll)
              .build(),
          )
          .ok();
        }
      });
      views
    };

    NestedViews { views }
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
    NotionFile::Empty => ViewLayout::Document,
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

#[derive(Default, Debug)]
pub struct CSVRelation(HashMap<String, PathBuf>);
impl Deref for CSVRelation {
  type Target = HashMap<String, PathBuf>;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

fn find_parent_child_csv_relationships(dir: &PathBuf) -> Result<CSVRelation, anyhow::Error> {
  let mut parent_csvs = Vec::new();
  let mut child_csvs = Vec::new();
  for entry in WalkDir::new(dir).into_iter().filter_map(Result::ok) {
    let path = entry.path();
    if path.is_file() && path.extension().and_then(|ext| ext.to_str()) == Some("csv") {
      if path
        .file_name()
        .map_or(false, |name| name.to_string_lossy().ends_with("_all.csv"))
      {
        parent_csvs.push(path.to_path_buf());
      } else {
        child_csvs.push(path.to_path_buf());
      }
    }
  }

  let mut csv_map: HashMap<String, PathBuf> = HashMap::new();
  for parent_csv in &parent_csvs {
    for child_csv in &child_csvs {
      // Check if the parent CSV contains all rows of the child CSV
      if is_csv_contained(parent_csv, child_csv).unwrap_or(false) {
        if let Some(child_csv_str) = child_csv.to_str() {
          let normalized_child_name = extract_file_name(child_csv_str);
          csv_map.insert(normalized_child_name, parent_csv.clone());
        }
      } else {
        println!(
          "{:?} is not contained in {:?}",
          child_csv.file_name(),
          parent_csv.file_name()
        );
      }
    }
  }

  Ok(CSVRelation(csv_map))
}
/// Checks if all rows of file_b are contained in file_a, ignoring paths, case, and whitespace.
/// If a row from file_b is not contained in file_a, prints both row_a and row_b.
pub fn is_csv_contained(file_a: &PathBuf, file_b: &PathBuf) -> Result<bool, Error> {
  // Read and collect rows from file_a into a Vec of HashSets for cell-level comparison
  let mut reader_a = Reader::from_path(file_a)?;
  let rows_a: Vec<HashSet<String>> = reader_a
    .records()
    .filter_map(|row| {
      row.ok().map(|r| {
        r.iter()
          .map(extract_file_name_from_view_ref)
          .collect::<HashSet<String>>()
      })
    })
    .collect();

  // Read and check each row of file_b
  let mut reader_b = Reader::from_path(file_b)?;
  let mut not_contained_rows = Vec::new();

  for row in reader_b.records() {
    let row_b: HashSet<String> = row?
      .iter()
      .filter(|cell| !cell.is_empty())
      .map(extract_file_name_from_view_ref)
      .collect();

    if row_b.is_empty() {
      continue;
    }

    let mut is_contained = false;
    for row_a in &rows_a {
      if row_b.is_subset(row_a) {
        is_contained = true;
        not_contained_rows.clear();
        break;
      }
    }

    if !is_contained {
      not_contained_rows.push(row_b);
      break;
    }
  }

  // If there are missing rows, print them and return false
  if !not_contained_rows.is_empty() {
    // for row in not_contained_rows {
    //   println!("{:?}", row);
    // }
    return Ok(false);
  }

  // All rows of file_b are contained in file_a
  Ok(true)
}

/// Helper function to normalize strings and extract the file name from a path.
fn extract_file_name_from_view_ref(input: &str) -> String {
  // Regex to match text before the parentheses
  let re = Regex::new(r"^(.*?)\s*\(").unwrap();
  if let Ok(Some(captures)) = re.captures(input) {
    if let Some(name) = captures.get(1) {
      return name.as_str().trim().to_string();
    }
  }
  input.to_string()
}

fn extract_file_name(input: &str) -> String {
  let normalized = input.trim().to_lowercase();
  let re = Regex::new(r"(?:.*/)?([^/()]+(?:\.[a-zA-Z0-9]+)?)").unwrap();
  if let Ok(Some(captures)) = re.captures(&normalized) {
    if let Some(path) = captures.get(1) {
      return PathBuf::from(path.as_str())
        .file_name()
        .map(|os_str| os_str.to_string_lossy().to_string())
        .unwrap_or(normalized);
    }
  }

  normalized
}

#[cfg(test)]
mod test_csv_relation {
  use super::*;
  use csv::Writer;
  use std::error::Error;
  use tempfile::{NamedTempFile, TempPath};

  #[test]
  fn test_is_csv_contained_all_rows_match() -> Result<(), Box<dyn Error>> {
    let header = vec!["Column1", "Column2", "Column3"];
    let file_a = create_temp_csv(vec![
      header.clone(),
      vec!["apple", "banana", "cherry"],
      vec!["dog", "elephant", "frog"],
      vec!["1", "2", "3"],
    ]);

    let file_b = create_temp_csv(vec![
      header.clone(),
      vec!["banana", "apple", "cherry"],
      vec!["dog", "frog", ""],
    ]);

    assert!(is_csv_contained(
      &file_a.to_path_buf(),
      &file_b.to_path_buf()
    )?);
    Ok(())
  }

  #[test]
  fn test_is_csv_contained_some_rows_missing() -> Result<(), Box<dyn Error>> {
    let header = vec!["Column1", "Column2", "Column3"];
    let file_a = create_temp_csv(vec![
      header.clone(),
      vec!["apple", "banana", "cherry"],
      vec!["dog", "elephant", "frog"],
    ]);

    let file_b = create_temp_csv(vec![
      header.clone(),
      vec!["banana", "apple", "cherry"],
      vec!["cat", "dog", "elephant"],
    ]);

    assert!(!is_csv_contained(
      &file_a.to_path_buf(),
      &file_b.to_path_buf()
    )?);
    Ok(())
  }

  #[test]
  fn test_is_csv_contained_empty_file_b() -> Result<(), Box<dyn Error>> {
    let header = vec!["Column1", "Column2", "Column3"];
    let file_a = create_temp_csv(vec![
      header.clone(),
      vec!["apple", "banana", "cherry"],
      vec!["dog", "elephant", "frog"],
    ]);

    let file_b = create_temp_csv(vec![]);

    assert!(is_csv_contained(
      &file_a.to_path_buf(),
      &file_b.to_path_buf()
    )?);
    Ok(())
  }

  #[test]
  fn test_is_csv_contained_no_overlap() -> Result<(), Box<dyn Error>> {
    let header = vec!["Column1", "Column2"];
    let file_a = create_temp_csv(vec![
      header.clone(),
      vec!["apple", "banana"],
      vec!["cat", "dog"],
    ]);

    let file_b = create_temp_csv(vec![
      header.clone(),
      vec!["elephant", ""],
      vec!["banana", ""],
    ]);
    assert!(!is_csv_contained(
      &file_a.to_path_buf(),
      &file_b.to_path_buf()
    )?);
    Ok(())
  }

  #[test]
  fn test_complex_csv_containment() {
    let header = vec!["Task", "Category", "Related Project", "Related Tasks"];
    let file_a = create_temp_csv(vec![
      header.clone(),
      vec![
        "Develop advertising plan",
        "Improvement, Marketing",
        "Marketing campaign (Projects%20&%20Tasks%20104d4deadd2c805fb3abcaab6d3727e7/Projects%2058b8977d6e4444a98ec4d64176a071e5/Marketing%20campaign%2088ac0cea4cb245efb44d63ace0a37d1e.md)",
        "Create social media plan (Projects%20&%20Tasks%20104d4deadd2c805fb3abcaab6d3727e7/Tasks%2076aaf8a4637542ed8175259692ca08bb/Create%20social%20media%20plan%204e70ea0b7d40427a9648bcf554a121f6.md), Create performance marketing plan (Projects%20&%20Tasks%20104d4deadd2c805fb3abcaab6d3727e7/Tasks%2076aaf8a4637542ed8175259692ca08bb/Create%20performance%20marketing%20plan%20b6aa6a9e9cc1446490984eaecc4930c7.md)",
      ],
    ]);

    // Create file_b with the provided content
    let file_b = create_temp_csv(vec![
      header.clone(),
      vec![
        "Create social media plan",
        "Develop advertising plan (../Tasks%2076aaf8a4637542ed8175259692ca08bb/Develop%20advertising%20plan%20a8e534ad763040029d0feb27fdb1820d.md)",
        "Marketing campaign (Marketing%20campaign%2088ac0cea4cb245efb44d63ace0a37d1e.md)",
        "Improvement, Marketing",
      ],
    ]);

    // Check if file_b is contained within file_a
    match is_csv_contained(&file_a.to_path_buf(), &file_b.to_path_buf()) {
      Ok(result) => {
        assert!(result, "File B should be contained in File A");
      },
      Err(e) => {
        panic!("Error occurred while checking CSV containment: {:?}", e);
      },
    }
  }

  /// Helper function to create a temporary CSV file with the given rows
  fn create_temp_csv(rows: Vec<Vec<&str>>) -> TempPath {
    let mut temp_file = NamedTempFile::new().unwrap();
    {
      let mut writer = Writer::from_writer(&mut temp_file);
      for row in rows {
        writer.write_record(row).unwrap();
      }
      writer.flush().unwrap();
    }
    temp_file.into_temp_path()
  }
}
