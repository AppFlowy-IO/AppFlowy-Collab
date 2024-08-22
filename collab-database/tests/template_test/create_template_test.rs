use crate::database_test::helper::create_database;
use assert_json_diff::assert_json_eq;
use collab_database::database::Database;
use collab_database::entity::CreateDatabaseParams;
use collab_database::rows::CreateRowParams;
use collab_database::template::builder::DatabaseTemplateBuilder;
use collab_database::template::entity::{
  create_database_from_template, DatabaseTemplate, FieldType,
};
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
    .create_field("name", FieldType::SingleSelect, false, |field_builder| {
      field_builder
        .create_cell("In Progress")
        .create_cell("Done")
        .create_cell("Not Started")
        .create_cell("In Progress")
        .create_cell("In Progress")
    })
    .build();

  assert_eq!(template.rows.len(), 5);
  assert_eq!(template.fields.len(), 2);

  let data = create_database_from_template(template);
  // let json = json!(data);
  // assert_json_eq!(json, json!(""));

  // let database = Database::create_with_view(database_data).await.unwrap();
}
