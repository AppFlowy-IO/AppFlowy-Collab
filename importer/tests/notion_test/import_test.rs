use crate::util::{print_view, unzip};
use assert_json_diff::assert_json_eq;
use collab_database::rows::Row;
use collab_document::document::gen_document_id;
use importer::notion::NotionImporter;
use nanoid::nanoid;

#[tokio::test]
async fn import_project_and_task_test() {
  let parent_dir = nanoid!(6);
  let (_cleaner, file_path) = unzip("project&task", &parent_dir).unwrap();
  let importer = NotionImporter::new(&file_path).unwrap();
  let imported_view = importer.import().await.unwrap();
  assert!(!imported_view.views.is_empty());
  assert_eq!(imported_view.name, "project&task");
  assert_eq!(imported_view.num_of_csv(), 2);
  assert_eq!(imported_view.num_of_markdown(), 1);

  /*
  - Projects & Tasks:Markdown
  - Tasks:CSV
  - Projects:CSV
  */
  let root_view = &imported_view.views[0].clone();
  assert_eq!(imported_view.views.len(), 1);
  assert_eq!(root_view.notion_name, "Projects & Tasks");

  assert_eq!(root_view.children.len(), 2);
  assert_eq!(root_view.children[0].notion_name, "Tasks");
  assert_eq!(root_view.children[1].notion_name, "Projects");

  let document_id = gen_document_id();
  let document = imported_view.views[0]
    .clone()
    .as_document(&document_id)
    .await
    .unwrap();

  // let json = document.to_json_value();
  // assert_json_eq!(json, json!(""));

  let project_database = imported_view.views[0].children[0]
    .clone()
    .as_database()
    .await
    .unwrap();
  let project_rows = project_database.collect_all_rows().await;
  assert_eq!(project_rows.len(), 17);

  let task_database = imported_view.views[0].children[1]
    .clone()
    .as_database()
    .await
    .unwrap();
  let task_rows = task_database.collect_all_rows().await;
  assert_eq!(task_rows.len(), 4);
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
