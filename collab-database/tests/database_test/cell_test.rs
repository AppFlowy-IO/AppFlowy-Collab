use collab_database::rows::Cells;

use crate::database_test::helper::create_database_with_default_data;
use crate::helper::{TestNumberCell, TestTextCell};

#[tokio::test]
async fn get_cells_for_field_test() {
  let database_id = uuid::Uuid::new_v4();
  let database_test = create_database_with_default_data(1, &database_id.to_string()).await;

  let cells = database_test.get_cells_for_field("v1", "f1", false).await;
  assert_eq!(cells.len(), 3);

  let cells = database_test.get_cells_for_field("v1", "f2", false).await;
  assert_eq!(cells.len(), 3);

  let cells = database_test.get_cells_for_field("v1", "f3", false).await;
  assert_eq!(cells.len(), 3);
}

#[tokio::test]
async fn get_cell_for_field_test() {
  let database_id = uuid::Uuid::new_v4();
  let database_test = create_database_with_default_data(1, &database_id.to_string()).await;
  let cell = database_test
    .get_cell("f1", &database_test.pre_define_row_ids[0])
    .await
    .cell
    .unwrap();
  let text_cell = TestTextCell::from(cell);
  assert_eq!(text_cell.0, "1f1cell");
}

#[tokio::test]
async fn update_cell_for_field_test() {
  let database_id = uuid::Uuid::new_v4();
  let mut database_test = create_database_with_default_data(1, &database_id.to_string()).await;
  let cells = database_test.get_cells_for_field("v1", "f1", false).await;
  assert_eq!(cells.len(), 3);

  let first_row_id = database_test.pre_define_row_ids[0].clone();
  database_test
    .update_row(first_row_id, |row_update| {
      row_update.update_cells(|cells_update| {
        cells_update.insert("f1", TestTextCell("hello world".to_string()));
      });
    })
    .await;

  let cells = database_test.get_cells_for_field("v1", "f1", false).await;
  assert_eq!(
    cells[0].cell.as_ref().unwrap().get("data").unwrap(),
    &"hello world".into()
  );
}

#[tokio::test]
async fn update_empty_cell_for_field_test() {
  let database_id = uuid::Uuid::new_v4();
  let mut database_test = create_database_with_default_data(1, &database_id.to_string()).await;
  let cells = database_test.get_cells_for_field("v1", "f2", false).await;
  assert_eq!(cells.len(), 3);

  let third_row_id = database_test.pre_define_row_ids[2].clone();
  database_test
    .update_row(third_row_id, |row_update| {
      row_update.update_cells(|cells_update| {
        cells_update.insert("f2", TestTextCell("hello world".to_string()));
      });
    })
    .await;

  let cells = database_test.get_cells_for_field("v1", "f2", false).await;
  assert_eq!(cells.len(), 3);
  assert_eq!(
    cells[2].cell.as_ref().unwrap().get("data").unwrap(),
    &"hello world".into()
  );
}

#[test]
fn cells_serde_test() {
  let mut cells = Cells::new();
  cells.insert("f1".to_string(), TestNumberCell(1).into());

  let json = serde_json::to_string(&cells).unwrap();
  let de_cells: Cells = serde_json::from_str(&json).unwrap();
  let cell = de_cells.get("f1").unwrap();
  let number_cell = TestNumberCell::from(cell);
  assert_eq!(number_cell.0, 1);
}
