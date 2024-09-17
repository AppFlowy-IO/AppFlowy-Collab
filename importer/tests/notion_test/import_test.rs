use crate::util::{print_view, unzip};
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

  /*
  - Projects & Tasks:Markdown
  - Tasks:CSV
  - Projects:CSV
  */
  assert_eq!(imported_view.views.len(), 1);
  assert_eq!(imported_view.views[0].notion_name, "Projects & Tasks");
  assert_eq!(imported_view.views[0].children.len(), 2);
  assert_eq!(imported_view.views[0].children[0].notion_name, "Tasks");
  assert_eq!(imported_view.views[0].children[1].notion_name, "Projects");

  let document_id = gen_document_id();
  let document = imported_view.views[0]
    .clone()
    .as_document(&document_id)
    .await
    .unwrap();

  // Projects & Task.md
  /*
    # Projects & Tasks
  - [Projects](Projects%20&%20Tasks%20104d4deadd2c805fb3abcaab6d3727e7/Projects%2058b8977d6e4444a98ec4d64176a071e5.md): This is your overview of all the projects in the pipeline
  - [Tasks](Projects%20&%20Tasks%20104d4deadd2c805fb3abcaab6d3727e7/Tasks%2076aaf8a4637542ed8175259692ca08bb.md)**:** This is your detailed breakdown of every task under your projects

  [Tasks](Projects%20&%20Tasks%20104d4deadd2c805fb3abcaab6d3727e7/Tasks%2076aaf8a4637542ed8175259692ca08bb.csv)

  ↓ Click through the different database tabs to see the same data in different ways

  Hover over any project name and click `◨ OPEN` to view more info and its associated tasks

  [Projects](Projects%20&%20Tasks%20104d4deadd2c805fb3abcaab6d3727e7/Projects%2058b8977d6e4444a98ec4d64176a071e5.csv)
    */

  // let json = document.to_json_value();
  // assert_json_eq!(json, json!(""));
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
