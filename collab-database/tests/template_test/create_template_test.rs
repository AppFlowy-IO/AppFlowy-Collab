use collab_database::template::builder::DatabaseTemplateBuilder;
use collab_database::template::entity::FieldType;
use collab_database::template::util::database_from_template;

#[tokio::test]
async fn create_template_test() {
  let expected_field_type = vec![
    FieldType::RichText,
    FieldType::SingleSelect,
    FieldType::MultiSelect,
    FieldType::DateTime,
    FieldType::Checklist,
    FieldType::LastEditedTime,
  ];

  let expected_cell_len = vec![6, 6, 6, 4, 2, 2];
  let expected_field_name = vec!["name", "status", "user", "time", "tasks", "last modified"];
  let expected_cell_data = vec![vec![
    "1th",
    "2th",
    "3th",
    "In Progress",
    "Done",
    "Not Started",
    "Lucas, Tom",
    "Lucas",
    "Tom",
    "",
    "Lucas, Tom, Nathan",
    "2024/08/22",
    "2024-08-22",
    "August 22, 2024",
    "2024-08-22 03:30 PM",
    "A",
    "B",
    "A",
    "1",
    "2",
    "3",
    "task1",
    "task2",
    "2024/08/22",
    "2024-08-22",
    "August 22, 2024",
    "2024-08-22 03:30 PM",
  ]];

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
  for (index, row) in template.rows.iter().enumerate() {
    assert_eq!(row.cells.len(), expected_cell_len[index]);
  }
  assert_eq!(template.fields.len(), 6);

  let database = database_from_template(template).await.unwrap();
  let fields = database.get_fields_in_view(database.get_inline_view_id().as_str(), None);
  assert_eq!(fields.len(), 6);
  for (index, field) in fields.iter().enumerate() {
    assert_eq!(field.field_type, expected_field_type[index].clone() as i64);
    assert_eq!(field.name, expected_field_name[index]);
  }

  let rows = database.get_all_rows().await;
  assert_eq!(rows.len(), 5);

  for field in fields {
    for (index, row) in rows.iter().enumerate() {
      assert_eq!(row.cells.len(), expected_cell_len[index]);
    }
  }
}
