use collab_database::database::{gen_database_id, Database};
use collab_database::template::csv::CSVTemplate;
use collab_database::template::entity::CELL_DATA;

#[tokio::test]
async fn import_csv_test() {
  let csv_data = include_str!("../asset/selected-services-march-2024-quarter-csv.csv");
  let csv_template = CSVTemplate::try_from(csv_data).unwrap();

  let database_id = gen_database_id();
  let database = Database::create_with_template(&database_id, csv_template)
    .await
    .unwrap();

  let fields = database.get_fields_in_view(&database.get_inline_view_id(), None);
  let rows = database.get_all_rows().await;
  let mut reader = csv::Reader::from_reader(csv_data.as_bytes());
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

  assert_eq!(rows.len(), 1200);
  assert_eq!(fields.len(), 14);

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
