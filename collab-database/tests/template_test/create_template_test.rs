use assert_json_diff::assert_json_eq;

use collab_database::database::{Database, DatabaseContext};
use collab_database::template::builder::DatabaseTemplateBuilder;
use collab_database::template::entity::{create_database_from_template, FieldType};
use serde_json::json;

#[tokio::test]
async fn create_template_test() {
  let template = DatabaseTemplateBuilder::new()
    .create_field("name", FieldType::RichText, true, |field_builder| {
      field_builder
        .create_cell("1th")
        .create_cell("2th")
        .create_cell("3th")
    })
    .create_field("status", FieldType::SingleSelect, false, |field_builder| {
      field_builder
        .create_cell("In Progress")
        .create_cell("Done")
        .create_cell("Not Started")
        .create_cell("In Progress")
        .create_cell("In Progress")
    })
    .create_field("user", FieldType::MultiSelect, false, |field_builder| {
      field_builder
        .create_cell("Lucas, Tom")
        .create_cell("Lucas")
        .create_cell("Tom")
        .create_cell("")
        .create_cell("Lucas, Tom, Nathan")
    })
    .create_field("time", FieldType::DateTime, false, |field_builder| {
      field_builder
        .create_cell("2024/08/22")
        .create_cell("2024-08-22")
        .create_cell("August 22, 2024")
        .create_cell("2024-08-22 03:30 PM")
    })
    .create_field("tasks", FieldType::Checklist, false, |field_builder| {
      field_builder
        .create_checklist_cell(vec!["A", "B"], vec!["A"])
        .create_checklist_cell(vec!["1", "2", "3"], Vec::<String>::new())
        .create_checklist_cell(vec!["task1", "task2"], vec!["task1", "task2"])
    })
    .create_field(
      "last modified",
      FieldType::LastEditedTime,
      false,
      |field_builder| {
        field_builder
          .create_cell("2024/08/22")
          .create_cell("2024-08-22")
          .create_cell("August 22, 2024")
          .create_cell("2024-08-22 03:30 PM")
      },
    )
    .build();

  assert_eq!(template.rows.len(), 5);
  assert_eq!(template.fields.len(), 6);

  let data = create_database_from_template(template);
  // let json = json!(data);
  // assert_json_eq!(json, json!(""));

  // let context = DatabaseContext {
  //   collab: (),
  //   collab_service: Arc::new(()),
  //   cloud_service: None,
  //   notifier: Default::default(),
  // };
  // let database = Database::create_with_view(data).await.unwrap();
}
