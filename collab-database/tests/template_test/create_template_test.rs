use collab_database::database::Database;
use crate::database_test::helper::create_database;
use collab_database::entity::CreateDatabaseParams;
use collab_database::rows::CreateRowParams;
use collab_database::template::builder::DatabaseTemplateBuilder;
use collab_database::template::entity::{DatabaseTemplate, FieldType};

#[tokio::test]
async fn create_template_test() {
  let database_data: CreateDatabaseParams = DatabaseTemplateBuilder::new()
    .create_field("name", FieldType::RichText, false, |field_builder| {
      field_builder
        .create_cell("Hello World")
        .create_cell("Hello Rust")
        .create_cell("Hello Dart")
    })
    .build()
    .into();

    let database = Database::create_with_view(database_data).await.unwrap();
}
