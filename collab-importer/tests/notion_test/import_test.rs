use crate::util::{parse_csv, setup_log, unzip_test_asset};
use collab::preclude::Collab;
use collab_database::database::Database;
use collab_database::entity::FieldType;
use collab_database::entity::FieldType::*;
use collab_database::error::DatabaseError;
use collab_database::fields::media_type_option::MediaCellData;
use collab_database::fields::{Field, StringifyTypeOption};
use collab_database::rows::Row;
use collab_document::blocks::{
  extract_page_id_from_block_delta, extract_view_id_from_block_data,
  mention_block_content_from_delta,
};

use collab_document::importer::define::{BlockType, URL_FIELD};
use collab_folder::hierarchy_builder::ParentChildViews;
use collab_folder::{default_folder_data, Folder, View};
use collab_importer::error::ImporterError;
use collab_importer::imported_collab::import_notion_zip_file;
use collab_importer::notion::page::NotionPage;
use collab_importer::notion::NotionImporter;
use futures::stream::StreamExt;
use percent_encoding::percent_decode_str;
use std::collections::{HashMap, HashSet};
use std::env::temp_dir;
use std::path::PathBuf;
use std::sync::Arc;

#[tokio::test]
async fn import_two_spaces_test2() {
  let (_cleaner, file_path) = unzip_test_asset("design").await.unwrap();
  let importer = NotionImporter::new(
    1,
    &file_path,
    uuid::Uuid::new_v4(),
    "http://test.appflowy.cloud".to_string(),
  )
  .unwrap();
  let info = importer.import().await.unwrap();
  let design_view = &info.views()[0];
  assert_eq!(design_view.notion_name, "Design");

  let all_views = design_view
    .get_linked_views()
    .into_iter()
    .map(|v| v.view_id)
    .collect::<Vec<_>>();

  let (document, _) = design_view.as_document().await.unwrap();
  let page_block_id = document.get_page_id().unwrap();
  let block_ids = document.get_block_children_ids(&page_block_id);
  let mut mention_blocks = HashSet::new();
  for block_id in block_ids.iter() {
    if let Some((_, deltas)) = document.get_block_delta(block_id) {
      println!("{:?}", deltas);
      mention_blocks.extend(
        deltas
          .into_iter()
          .filter_map(|delta| mention_block_content_from_delta(&delta))
          .collect::<Vec<_>>(),
      )
    }
  }

  assert_eq!(mention_blocks.len(), 3);
  // the mention pages should be included in the all_views
  mention_blocks.retain(|block| !all_views.contains(&block.page_id));
  assert!(mention_blocks.is_empty());

  let collabs = info.into_collab_stream().await.collect::<Vec<_>>().await;
  assert!(!collabs.is_empty())
}

#[tokio::test]
async fn import_two_spaces_test() {
  let (_cleaner, file_path) = unzip_test_asset("two_spaces").await.unwrap();
  let importer = NotionImporter::new(
    1,
    &file_path,
    uuid::Uuid::new_v4(),
    "http://test.appflowy.cloud".to_string(),
  )
  .unwrap();
  let info = importer.import().await.unwrap();
  assert!(!info.views().is_empty());
  assert_eq!(info.name, "two_spaces");

  let first_space = &info.views()[0];
  assert_eq!(first_space.notion_name, "space one");
  assert!(first_space.is_dir);
  assert_eq!(first_space.children.len(), 1);
  let blog_post_page = &first_space.children[0];
  assert_blog_post(&info.host, &info.workspace_id, blog_post_page).await;

  let second_space = info.views()[1].clone();
  assert_eq!(second_space.notion_name, "space two");
  assert!(second_space.is_dir);
  assert_eq!(second_space.children.len(), 1);
  let project_and_task = &second_space.children[0];
  assert_project_and_task(project_and_task).await;

  let views: Vec<ParentChildViews> = info.build_nested_views().await.into_inner();
  for view in views {
    assert!(view.view.space_info().is_some());
  }
}

#[tokio::test]
async fn import_blog_post_document_test() {
  setup_log();
  let workspace_id = uuid::Uuid::new_v4();
  let (_cleaner, file_path) = unzip_test_asset("blog_post").await.unwrap();
  let host = "http://test.appflowy.cloud";
  let importer = NotionImporter::new(1, &file_path, workspace_id, host.to_string()).unwrap();
  let info = importer.import().await.unwrap();
  assert_eq!(info.name, "blog_post");
  assert_eq!(info.num_of_csv(), 0);
  assert_eq!(info.num_of_markdown(), 1);

  let root_view = &info.views()[0];
  assert_blog_post(host, &info.workspace_id, root_view).await;
}

#[tokio::test]
async fn import_project_and_task_test() {
  let workspace_id = uuid::Uuid::new_v4();
  let (_cleaner, file_path) = unzip_test_asset("project&task").await.unwrap();
  let importer = NotionImporter::new(
    1,
    &file_path,
    workspace_id,
    "http://test.appflowy.cloud".to_string(),
  )
  .unwrap();
  let import = importer.import().await.unwrap();
  println!(
    "workspace_id:{}, views:\n{}",
    workspace_id,
    import.build_nested_views().await
  );
  assert!(!import.views().is_empty());
  assert_eq!(import.name, "project&task");
  assert_eq!(import.num_of_csv(), 2);
  assert_eq!(import.num_of_markdown(), 1);
  assert_eq!(import.views().len(), 1);

  /*
  - Projects & Tasks: Markdown
  - Tasks: CSV
  - Projects: CSV
  */
  let root_view = &import.views()[0];
  assert_project_and_task(root_view).await;
}

#[tokio::test]
async fn import_empty_zip_test() {
  let workspace_id = uuid::Uuid::new_v4();
  let (_cleaner, file_path) = unzip_test_asset("empty_zip").await.unwrap();
  let importer = NotionImporter::new(
    1,
    &file_path,
    workspace_id,
    "http://test.appflowy.cloud".to_string(),
  )
  .unwrap();
  let err = importer.import().await.unwrap_err();
  assert!(matches!(err, ImporterError::CannotImport));
}

#[tokio::test]
async fn import_project_and_task_collab_test() {
  let workspace_id = uuid::Uuid::new_v4().to_string();
  let host = "http://test.appflowy.cloud";
  let zip_file_path = PathBuf::from("./tests/asset/project&task.zip");
  let temp_dir = temp_dir().join(uuid::Uuid::new_v4().to_string());
  std::fs::create_dir_all(&temp_dir).unwrap();
  let info = import_notion_zip_file(1, host, &workspace_id, zip_file_path, temp_dir.clone())
    .await
    .unwrap();

  assert_eq!(info.len(), 4);
  assert_eq!(info[0].name, "project&task");
  assert_eq!(info[0].collabs.len(), 1);
  assert_eq!(info[0].resource.files.len(), 0);

  assert_eq!(info[1].name, "Projects & Tasks");
  assert_eq!(info[1].collabs.len(), 1);
  assert_eq!(info[1].resource.files.len(), 0);

  assert_eq!(info[2].name, "Projects");
  assert_eq!(info[2].collabs.len(), 5);
  assert_eq!(info[2].resource.files.len(), 2);
  assert_eq!(info[2].file_size(), 1143952);

  assert_eq!(info[3].name, "Tasks");
  assert_eq!(info[3].collabs.len(), 18);
  assert_eq!(info[3].resource.files.len(), 0);

  println!("{info}");
}

async fn assert_project_and_task(root_view: &NotionPage) {
  assert_eq!(root_view.notion_name, "Projects & Tasks");
  let linked_views = root_view.get_linked_views();
  check_project_and_task_document(root_view, linked_views.clone()).await;

  assert_eq!(linked_views.len(), 2);
  assert_eq!(linked_views[0].notion_name, "Tasks");
  assert_eq!(linked_views[1].notion_name, "Projects");

  check_task_database(&linked_views[0]).await;
  check_project_database(&linked_views[1]).await;
}

async fn assert_blog_post(host: &str, workspace_id: &str, root_view: &NotionPage) {
  let object_id = root_view.view_id.clone();

  let mut expected_urls = vec![
    "PGTRCFsf2duc7iP3KjE62Xs8LE7B96a0aQtLtGtfIcw=.jpg",
    "fFWPgqwdqbaxPe7Q_vUO143Sa2FypnRcWVibuZYdkRI=.jpg",
    "EIj9Z3yj8Gw8UW60U8CLXx7ulckEs5Eu84LCFddCXII=.jpg",
  ]
  .into_iter()
  .map(|s| format!("{host}/{workspace_id}/v1/blob/{object_id}/{s}"))
  .collect::<Vec<String>>();

  let (document, _) = root_view.as_document().await.unwrap();
  let page_block_id = document.get_page_id().unwrap();
  let block_ids = document.get_block_children_ids(&page_block_id);
  for block_id in block_ids.iter() {
    if let Some((block_type, block_data)) = document.get_block_data(block_id) {
      if matches!(block_type, BlockType::Image) {
        let url = block_data.get(URL_FIELD).unwrap().as_str().unwrap();
        expected_urls.retain(|allowed_url| !url.contains(allowed_url));
      }
    }
  }
  assert!(expected_urls.is_empty());
}

async fn check_project_and_task_document(
  document_view: &NotionPage,
  notion_views: Vec<NotionPage>,
) {
  let (document, _) = document_view.as_document().await.unwrap();
  let first_block_id = document.get_page_id().unwrap();
  let block_ids = document.get_block_children_ids(&first_block_id);

  let mut cloned_notion_views = notion_views.clone();
  for block_id in block_ids.iter() {
    if let Some((block_type, block_delta)) = document.get_block_delta(block_id) {
      if matches!(block_type, BlockType::BulletedList) {
        let page_id = extract_page_id_from_block_delta(&block_delta).unwrap();
        cloned_notion_views.retain(|view| view.view_id != page_id);
      }
    }
  }

  let mut cloned_notion_views2 = notion_views.clone();
  for block_id in block_ids.iter() {
    if let Some((block_type, data)) = document.get_block_data(block_id) {
      if matches!(block_type, BlockType::Paragraph) {
        if let Some(view_id) = extract_view_id_from_block_data(&data) {
          cloned_notion_views2.retain(|view| view.view_id != view_id);
        }
      }
    }
  }

  assert!(cloned_notion_views.is_empty());
  assert!(cloned_notion_views2.is_empty());
}

async fn check_task_database(linked_view: &NotionPage) {
  assert_eq!(linked_view.notion_name, "Tasks");

  let (csv_fields, csv_rows) = parse_csv(linked_view.notion_file.imported_file_path().unwrap());
  let (database, _) = linked_view.as_database().await.unwrap();
  let views = database.get_all_views();
  assert_eq!(views.len(), 1);
  assert_eq!(linked_view.view_id, views[0].id);

  let fields = database.get_fields_in_view(&database.get_inline_view_id(), None);
  let rows = database.collect_all_rows().await;
  assert_eq!(rows.len(), 17);
  assert_eq!(fields.len(), csv_fields.len());
  assert_eq!(fields.len(), 13);

  let expected_file_type = vec![
    RichText,
    SingleSelect,
    SingleSelect,
    DateTime,
    SingleSelect,
    MultiSelect,
    SingleSelect,
    SingleSelect,
    RichText,
    RichText,
    RichText,
    DateTime,
    Number,
  ];
  for (index, field) in fields.iter().enumerate() {
    assert_eq!(FieldType::from(field.field_type), expected_file_type[index]);
    // println!("{:?}", FieldType::from(field.field_type));
  }
  for (index, field) in csv_fields.iter().enumerate() {
    assert_eq!(&fields[index].name, field);
  }

  assert_database_rows_with_csv_rows(csv_rows, database, fields, rows, HashMap::new());
}

async fn check_project_database(linked_view: &NotionPage) {
  assert_eq!(linked_view.notion_name, "Projects");

  let upload_files = linked_view.notion_file.upload_files();
  assert_eq!(upload_files.len(), 2);

  let (csv_fields, csv_rows) = parse_csv(linked_view.notion_file.imported_file_path().unwrap());
  let (database, _) = linked_view.as_database().await.unwrap();
  let fields = database.get_fields_in_view(&database.get_inline_view_id(), None);
  let rows = database.collect_all_rows().await;
  assert_eq!(rows.len(), 4);
  assert_eq!(fields.len(), csv_fields.len());
  assert_eq!(fields.len(), 13);

  let expected_file_type = vec![
    RichText,
    SingleSelect,
    SingleSelect,
    MultiSelect,
    SingleSelect,
    Number,
    RichText,
    RichText,
    RichText,
    MultiSelect,
    Number,
    Checkbox,
    Media,
  ];
  for (index, field) in fields.iter().enumerate() {
    assert_eq!(FieldType::from(field.field_type), expected_file_type[index]);
    // println!("{:?}", FieldType::from(field.field_type));
  }
  for (index, field) in csv_fields.iter().enumerate() {
    assert_eq!(&fields[index].name, field);
  }
  let  expected_files = HashMap::from([("DO010003572.jpeg", "http://test.appflowy.cloud/ef151418-41b1-4ca2-b190-3ed59a3bea76/v1/blob/ysINEn/TZQyERYXrrBq25cKsZVAvRqe9ZPTYNlG8EJfUioKruI=.jpeg"), ("appflowy_2x.png", "http://test.appflowy.cloud/ef151418-41b1-4ca2-b190-3ed59a3bea76/v1/blob/ysINEn/c9Ju1jv95fPw6irxJACDKPDox_-hfd-3_blIEapMaZc=.png"),]);
  assert_database_rows_with_csv_rows(csv_rows, database, fields, rows, expected_files);
}

fn assert_database_rows_with_csv_rows(
  csv_rows: Vec<Vec<String>>,
  database: Database,
  fields: Vec<Field>,
  rows: Vec<Result<Row, DatabaseError>>,
  mut expected_files: HashMap<&str, &str>,
) {
  let type_option_by_field_id = fields
    .iter()
    .map(|field| {
      (
        field.id.clone(),
        match database.get_stringify_type_option(&field.id) {
          None => {
            panic!("Field {:?} doesn't have type option", field)
          },
          Some(ty) => ty,
        },
      )
    })
    .collect::<HashMap<String, Box<dyn StringifyTypeOption>>>();

  for (row_index, row) in rows.into_iter().enumerate() {
    let row = row.unwrap();
    assert_eq!(row.cells.len(), fields.len());
    for (field_index, field) in fields.iter().enumerate() {
      let cell = row.cells.get(&field.id).unwrap();
      let field_type = FieldType::from(field.field_type);
      let type_option = type_option_by_field_id[&field.id].as_ref();
      let cell_data = type_option.stringify_cell(cell);

      if matches!(field_type, FieldType::Media) {
        let mut data = MediaCellData::from(cell);
        if let Some(file) = data.files.pop() {
          expected_files.remove(file.name.as_str()).unwrap();
        }
      } else {
        assert_eq!(
          cell_data,
          percent_decode_str(&csv_rows[row_index][field_index])
            .decode_utf8()
            .unwrap()
            .to_string(),
          "current:{}, expected:{}\nRow: {}, Field: {}, type: {:?}",
          cell_data,
          csv_rows[row_index][field_index],
          row_index,
          field.name,
          FieldType::from(field.field_type)
        );
      }
    }
  }

  assert!(expected_files.is_empty());
}

#[tokio::test]
async fn import_level_test() {
  let (_cleaner, file_path) = unzip_test_asset("import_test").await.unwrap();
  let importer = NotionImporter::new(
    1,
    &file_path,
    uuid::Uuid::new_v4(),
    "http://test.appflowy.cloud".to_string(),
  )
  .unwrap();
  let info = importer.import().await.unwrap();
  assert!(!info.views().is_empty());
  assert_eq!(info.name, "import_test");

  let uid = 1;
  let collab = Collab::new(uid, &info.workspace_id, "1", vec![], false);
  let mut folder = Folder::create(1, collab, None, default_folder_data(&info.workspace_id));

  let view_hierarchy = info.build_nested_views().await;
  assert_eq!(view_hierarchy.flatten_views().len(), 14);
  folder.insert_nested_views(view_hierarchy.into_inner());

  let first_level_views = folder.get_views_belong_to(&info.workspace_id);
  assert_eq!(first_level_views.len(), 1);
  assert_eq!(first_level_views[0].children.len(), 3);
  println!("first_level_views: {:?}", first_level_views);

  let second_level_views = folder.get_views_belong_to(&first_level_views[0].id);
  verify_first_level_views(&second_level_views, &mut folder);

  // Print out the views for debugging or manual inspection
  /*
  - import_test
    - Root2:Markdown
      - root2-link:Markdown
    - Home:Markdown
      - Home views:Markdown
      - My tasks:Markdown
    - Root:Markdown
      - root-2:Markdown
        - root-2-1:Markdown
          - root-2-database:CSV
      - root-1:Markdown
        - root-1-1:Markdown
      - root 3:Markdown
        - root 3 1:Markdown
  */
  let nested_view = info.build_nested_views().await;
  println!("{}", nested_view);
}

#[tokio::test]
async fn import_empty_space() {
  let (_cleaner, file_path) = unzip_test_asset("empty_spaces").await.unwrap();
  let importer = NotionImporter::new(
    1,
    &file_path,
    uuid::Uuid::new_v4(),
    "http://test.appflowy.cloud".to_string(),
  )
  .unwrap();
  let info = importer.import().await.unwrap();
  assert!(!info.views().is_empty());
  assert_eq!(info.name, "empty_spaces");

  let view_hierarchy = info.build_nested_views().await;
  println!("{}", view_hierarchy);
  let views: Vec<ParentChildViews> = view_hierarchy.into_inner();
  assert_eq!(views.len(), 2);

  // only the first level views will be the space view
  assert!(views[0].view.space_info().is_some());
  let second_space = views[0].clone();
  assert_eq!(second_space.view.name, "second space");
  assert_eq!(second_space.children.len(), 2);
  assert!(second_space.children[0].view.space_info().is_none());
  assert_eq!(second_space.children[0].view.name, "1");
  assert!(second_space.children[1].view.space_info().is_none());
  assert_eq!(second_space.children[1].view.name, "2");

  let first_space = views[1].clone();
  assert!(first_space.view.space_info().is_some());
  assert_eq!(first_space.view.name, "first space");
}

// Helper function to verify second and third level views based on the first level view name
fn verify_first_level_views(first_level_views: &[Arc<View>], folder: &mut Folder) {
  for view in first_level_views {
    let second_level_views = folder.get_views_belong_to(&view.id);
    match view.name.as_str() {
      "Root2" => {
        assert_eq!(second_level_views.len(), 1);
        assert_eq!(second_level_views[0].name, "root2-link");
      },
      "Home" => {
        assert_eq!(second_level_views.len(), 2);
        assert_eq!(second_level_views[0].name, "Home views");
        assert_eq!(second_level_views[1].name, "My tasks");
      },
      "Root" => {
        assert_eq!(second_level_views.len(), 3);
        verify_root_second_level_views(&second_level_views, folder);
      },
      _ => panic!("Unexpected view name: {}", view.name),
    }
  }
}

// Helper function to verify third level views based on the second level view name under "Root"
fn verify_root_second_level_views(second_level_views: &[Arc<View>], folder: &mut Folder) {
  for view in second_level_views {
    let third_level_views = folder.get_views_belong_to(&view.id);
    match view.name.as_str() {
      "root-2" => {
        assert_eq!(third_level_views.len(), 1);
        assert_eq!(third_level_views[0].name, "root-2-1");
      },
      "root-1" => {
        assert_eq!(third_level_views.len(), 1);
        assert_eq!(third_level_views[0].name, "root-1-1");
      },
      "root 3" => {
        assert_eq!(third_level_views.len(), 1);
        assert_eq!(third_level_views[0].name, "root 3 1");
      },
      _ => panic!("Unexpected second level view name: {}", view.name),
    }
  }
}
