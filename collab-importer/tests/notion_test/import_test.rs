use crate::util::{parse_csv, print_view, setup_log, unzip};

use collab_database::template::entity::CELL_DATA;
use collab_document::blocks::{extract_page_id_from_block_delta, extract_view_id_from_block_data};
use collab_document::importer::define::{BlockType, URL_FIELD};
use collab_importer::notion::page::NotionView;
use collab_importer::notion::NotionImporter;
use nanoid::nanoid;
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};

#[tokio::test]
async fn import_blog_post_document_test() {
  setup_log();
  let parent_dir = nanoid!(6);
  let workspace_id = uuid::Uuid::new_v4();
  let (_cleaner, file_path) = unzip("blog_post", &parent_dir).unwrap();
  let host = "http://test.appflowy.cloud";
  let importer = NotionImporter::new(&file_path, workspace_id, host.to_string()).unwrap();
  let imported_view = importer.import().await.unwrap();
  assert_eq!(imported_view.name, "blog_post");
  assert_eq!(imported_view.num_of_csv(), 0);
  assert_eq!(imported_view.num_of_markdown(), 1);

  let root_view = &imported_view.views[0];
  let external_link_views = root_view.get_external_link_notion_view();
  let object_id = utf8_percent_encode(&root_view.object_id, NON_ALPHANUMERIC).to_string();

  let mut expected_urls = vec![
    "PGTRCFsf2duc7iP3KjE62Xs8LE7B96a0aQtLtGtfIcw=.jpg",
    "fFWPgqwdqbaxPe7Q_vUO143Sa2FypnRcWVibuZYdkRI=.jpg",
    "EIj9Z3yj8Gw8UW60U8CLXx7ulckEs5Eu84LCFddCXII=.jpg",
  ]
  .into_iter()
  .map(|s| format!("{host}/{workspace_id}/v1/blob/{object_id}/{s}"))
  .collect::<Vec<String>>();

  let size = root_view.get_payload_size_recursively();
  assert_eq!(size, 5333956);

  let document = root_view.as_document(external_link_views).await.unwrap();
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

  println!("Allowed URLs: {:?}", expected_urls);
  assert!(expected_urls.is_empty());
}

#[tokio::test]
async fn import_project_and_task_test() {
  let parent_dir = nanoid!(6);
  let workspace_id = uuid::Uuid::new_v4();
  let (_cleaner, file_path) = unzip("project&task", &parent_dir).unwrap();
  let importer = NotionImporter::new(
    &file_path,
    workspace_id,
    "http://test.appflowy.cloud".to_string(),
  )
  .unwrap();
  let imported_view = importer.import().await.unwrap();
  assert!(!imported_view.views.is_empty());
  assert_eq!(imported_view.name, "project&task");
  assert_eq!(imported_view.num_of_csv(), 2);
  assert_eq!(imported_view.num_of_markdown(), 1);

  /*
  - Projects & Tasks: Markdown
  - Tasks: CSV
  - Projects: CSV
  */
  let root_view = &imported_view.views[0];
  assert_eq!(root_view.notion_name, "Projects & Tasks");
  assert_eq!(imported_view.views.len(), 1);
  assert_eq!(root_view.get_payload_size_recursively(), 1156965);
  let linked_views = root_view.get_linked_views();
  check_project_and_task_document(root_view, linked_views.clone()).await;

  assert_eq!(linked_views.len(), 2);
  assert_eq!(linked_views[0].notion_name, "Tasks");
  assert_eq!(linked_views[1].notion_name, "Projects");

  check_database_view(&linked_views[0], "Tasks", 17, 13).await;
  check_database_view(&linked_views[1], "Projects", 4, 12).await;
}

async fn check_project_and_task_document(
  document_view: &NotionView,
  notion_views: Vec<NotionView>,
) {
  let external_link_views = document_view.get_external_link_notion_view();
  let document = document_view
    .as_document(external_link_views)
    .await
    .unwrap();
  let first_block_id = document.get_page_id().unwrap();
  let block_ids = document.get_block_children_ids(&first_block_id);

  let mut cloned_notion_views = notion_views.clone();
  for block_id in block_ids.iter() {
    if let Some((block_type, block_delta)) = document.get_block_delta(block_id) {
      if matches!(block_type, BlockType::BulletedList) {
        let page_id = extract_page_id_from_block_delta(&block_delta).unwrap();
        cloned_notion_views.retain(|view| view.object_id != page_id);
      }
    }
  }

  let mut cloned_notion_views2 = notion_views.clone();
  for block_id in block_ids.iter() {
    if let Some((block_type, data)) = document.get_block_data(block_id) {
      if matches!(block_type, BlockType::Paragraph) {
        if let Some(view_id) = extract_view_id_from_block_data(&data) {
          cloned_notion_views2.retain(|view| view.object_id != view_id);
        }
      }
    }
  }

  assert!(cloned_notion_views.is_empty());
  assert!(cloned_notion_views2.is_empty());
}

async fn check_database_view(
  linked_view: &NotionView,
  expected_name: &str,
  expected_rows_count: usize,
  expected_fields_count: usize,
) {
  assert_eq!(linked_view.notion_name, expected_name);

  let (csv_fields, csv_rows) = parse_csv(linked_view.notion_file.imported_file_path().unwrap());
  let database = linked_view.as_database().await.unwrap();
  let fields = database.get_fields_in_view(&database.get_inline_view_id(), None);
  let rows = database.collect_all_rows().await;
  assert_eq!(rows.len(), expected_rows_count);
  assert_eq!(fields.len(), csv_fields.len());
  assert_eq!(fields.len(), expected_fields_count);

  for (index, field) in csv_fields.iter().enumerate() {
    assert_eq!(&fields[index].name, field);
  }
  for (row_index, row) in rows.into_iter().enumerate() {
    let row = row.unwrap();
    assert_eq!(row.cells.len(), fields.len());
    for (field_index, field) in fields.iter().enumerate() {
      let cell = row
        .cells
        .get(&field.id)
        .unwrap()
        .get(CELL_DATA)
        .cloned()
        .unwrap();
      let cell_data = cell.cast::<String>().unwrap();
      assert_eq!(
        cell_data, csv_rows[row_index][field_index],
        "Row: {}, Field: {}:{}",
        row_index, field.name, field_index
      );
    }
  }
}

#[tokio::test]
async fn test_importer() {
  let parent_dir = nanoid!(6);
  let (_cleaner, file_path) = unzip("import_test", &parent_dir).unwrap();
  let importer = NotionImporter::new(
    &file_path,
    uuid::Uuid::new_v4(),
    "http://test.appflowy.cloud".to_string(),
  )
  .unwrap();
  let imported_view = importer.import().await.unwrap();
  assert!(!imported_view.views.is_empty());
  assert_eq!(imported_view.name, "import_test");

  /*
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
  for view in imported_view.views {
    print_view(&view, 0);
  }
}
