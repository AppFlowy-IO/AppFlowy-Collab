use crate::helper::make_rocks_db;
use crate::user_test::helper::TestUserDatabaseServiceImpl;
use collab::core::collab::default_client_id;
use collab_database::database::{Database, gen_database_id, gen_database_view_id};
use collab_database::entity::FieldType;
use collab_database::rows::Row;
use collab_database::template::builder::DatabaseTemplateBuilder;
use collab_database::template::entity::CELL_DATA;
use futures::StreamExt;
use std::sync::Arc;
use uuid::Uuid;

#[tokio::test]
async fn create_template_test() {
  let workspace_id = Uuid::new_v4().to_string();
  let database_id = gen_database_id();
  let expected_field_type = [
    FieldType::RichText,
    FieldType::SingleSelect,
    FieldType::MultiSelect,
    FieldType::DateTime,
    FieldType::Checklist,
    FieldType::LastEditedTime,
  ];

  let expected_cell_len = [6, 6, 6, 4, 2, 2];
  let expected_field_name = ["name", "status", "user", "time", "tasks", "last modified"];

  let template = DatabaseTemplateBuilder::new(database_id.clone(), gen_database_view_id(), None)
    .create_field(
      &None,
      &database_id,
      "name",
      FieldType::RichText,
      true,
      |field_builder| {
        field_builder
          .create_cell("1th")
          .create_cell("2th")
          .create_cell("3th")
      },
    )
    .await
    .create_field(
      &None,
      &database_id,
      "status",
      FieldType::SingleSelect,
      false,
      |field_builder| {
        field_builder
          .create_cell("In Progress")
          .create_cell("Done")
          .create_cell("Not Started")
          .create_cell("In Progress")
          .create_cell("In Progress")
      },
    )
    .await
    .create_field(
      &None,
      &database_id,
      "user",
      FieldType::MultiSelect,
      false,
      |field_builder| {
        field_builder
          .create_cell("Lucas, Tom")
          .create_cell("Lucas")
          .create_cell("Tom")
          .create_cell("")
          .create_cell("Lucas, Tom, Nathan")
      },
    )
    .await
    .create_field(
      &None,
      &database_id,
      "time",
      FieldType::DateTime,
      false,
      |field_builder| {
        field_builder
          .create_cell("2024/08/22")
          .create_cell("2024-08-22")
          .create_cell("August 22, 2024")
          .create_cell("2024-08-22 03:30 PM")
      },
    )
    .await
    .create_field(
      &None,
      &database_id,
      "tasks",
      FieldType::Checklist,
      false,
      |field_builder| {
        field_builder
          .create_checklist_cell(vec!["A", "B"], vec!["A"])
          .create_checklist_cell(vec!["1", "2", "3"], Vec::<String>::new())
          .create_checklist_cell(vec!["task1", "task2"], vec!["task1", "task2"])
      },
    )
    .await
    .create_field(
      &None,
      &database_id,
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
    .await
    .build();

  assert_eq!(template.rows.len(), 5);
  for (index, row) in template.rows.iter().enumerate() {
    assert_eq!(row.cells.len(), expected_cell_len[index]);
  }
  assert_eq!(template.fields.len(), 6);
  let db = make_rocks_db();
  let service = Arc::new(TestUserDatabaseServiceImpl::new(
    1,
    workspace_id,
    db,
    default_client_id(),
  ));
  let database = Database::create_with_template(template, service.clone(), service)
    .await
    .unwrap();

  // Assert num of fields
  let fields = database.get_fields_in_view(
    database.get_first_database_view_id().unwrap().as_str(),
    None,
  );
  assert_eq!(fields.len(), 6);
  for (index, field) in fields.iter().enumerate() {
    assert_eq!(field.field_type, expected_field_type[index] as i64);
    assert_eq!(field.name, expected_field_name[index]);
  }

  // Assert num of rows
  let rows: Vec<Row> = database
    .get_all_rows(10, None, false)
    .await
    .filter_map(|result| async move { result.ok() })
    .collect()
    .await;
  assert_eq!(rows.len(), 5);
  for row in rows.iter() {
    for field in &fields {
      let cell = row
        .cells
        .get(&field.id)
        .and_then(|cell| cell.get(CELL_DATA).cloned());
      println!("data: {:?}", cell);
    }
    println!("\n");
  }
}
