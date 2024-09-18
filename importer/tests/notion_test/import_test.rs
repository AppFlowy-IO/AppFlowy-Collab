use crate::util::{parse_csv, print_view, unzip};

use collab_database::template::entity::CELL_DATA;
use collab_document::document::gen_document_id;
use importer::notion::{NotionImporter, NotionView};
use nanoid::nanoid;

#[tokio::test]
async fn import_project_and_task_test2() {
  let parent_dir = nanoid!(6);
  let (_cleaner, file_path) = unzip("project&task", &parent_dir).unwrap();
  let importer = NotionImporter::new(&file_path).unwrap();
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
  check_document(&root_view, "Projects & Tasks".to_string()).await;

  let linked_views = root_view.get_linked_views();
  assert_eq!(linked_views.len(), 2);
  assert_eq!(linked_views[0].notion_name, "Tasks");
  assert_eq!(linked_views[1].notion_name, "Projects");
  println!("linked_views: {:?}", linked_views);

  check_database_view(&linked_views[0], "Tasks", 17, 13).await;
  check_database_view(&linked_views[1], "Projects", 4, 11).await;
}

async fn check_document(document_view: &NotionView, expected: String) {
  let document_id = gen_document_id();
  let document = document_view.as_document(&document_id).await.unwrap();
}

async fn check_database_view(
  linked_view: &NotionView,
  expected_name: &str,
  expected_rows_count: usize,
  expected_fields_count: usize,
) {
  assert_eq!(linked_view.notion_name, expected_name);

  let (csv_fields, csv_rows) = parse_csv(linked_view.notion_file.file_path().unwrap());
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
  let importer = NotionImporter::new(&file_path).unwrap();
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
