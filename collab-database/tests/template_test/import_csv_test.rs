use crate::helper::make_rocks_db;
use crate::user_test::helper::TestUserDatabaseServiceImpl;
use collab::core::collab::default_client_id;
use collab_database::database::Database;
use collab_database::rows::Row;
use collab_database::template::csv::CSVTemplate;
use collab_database::template::entity::CELL_DATA;
use futures::StreamExt;
use std::sync::Arc;
use uuid::Uuid;

#[tokio::test]
async fn import_csv_test() {
  let csv_data = include_str!("../asset/selected-services-march-2024-quarter-csv.csv");
  let csv_template = CSVTemplate::try_from_reader(csv_data.as_bytes(), false, None).unwrap();

  let workspace_id = Uuid::new_v4().to_string();
  let database_template = csv_template.try_into_database_template(None).await.unwrap();
  let db = make_rocks_db();
  let service = Arc::new(TestUserDatabaseServiceImpl::new(
    1,
    workspace_id,
    db,
    default_client_id(),
  ));
  let database = Database::create_with_template(database_template, service.clone(), service)
    .await
    .unwrap();

  let fields = database.get_fields_in_view(&database.get_first_database_view_id().unwrap(), None);
  let rows: Vec<Row> = database
    .get_all_rows(20, None, false)
    .await
    .filter_map(|result| async move { result.ok() })
    .collect()
    .await;

  let mut reader = csv::Reader::from_reader(csv_data.as_bytes());
  let csv_fields = reader
    .headers()
    .unwrap()
    .iter()
    .map(|s| s.to_string())
    .collect::<Vec<String>>();
  let csv_rows = reader
    .records()
    .flat_map(|r| r.ok())
    .map(|record| {
      record
        .into_iter()
        .map(|s| s.to_string())
        .collect::<Vec<String>>()
    })
    .collect::<Vec<Vec<String>>>();

  assert_eq!(rows.len(), csv_rows.len());
  assert_eq!(rows.len(), 1200);

  assert_eq!(fields.len(), csv_fields.len());
  assert_eq!(fields.len(), 14);

  for (index, field) in fields.iter().enumerate() {
    assert_eq!(field.name, csv_fields[index]);
  }

  for (row_index, row) in rows.iter().enumerate() {
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
      )
    }
  }
}
