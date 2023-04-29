use crate::database_test::helper::create_database_with_default_data;
use crate::helper::TestTextCell;
use collab::core::any_map::AnyMapExtension;

#[test]
fn get_cells_for_field_test() {
  let database_test = create_database_with_default_data(1, "1");

  let cells = database_test.get_cells_for_field("v1", "f1");
  assert_eq!(cells.len(), 3);

  let cells = database_test.get_cells_for_field("v1", "f2");
  assert_eq!(cells.len(), 2);

  let cells = database_test.get_cells_for_field("v1", "f3");
  assert_eq!(cells.len(), 2);
}

#[test]
fn get_cell_for_field_test() {
  let database_test = create_database_with_default_data(1, "1");
  let cell = database_test.get_cell("f1", 1).unwrap().cell;
  let text_cell = TestTextCell::from(cell);
  assert_eq!(text_cell.0, "1f1cell");
}

#[test]
fn update_cell_for_field_test() {
  let database_test = create_database_with_default_data(1, "1");
  let cells = database_test.get_cells_for_field("v1", "f1");
  assert_eq!(cells.len(), 3);

  database_test.update_row(1, |row_update| {
    row_update.update_cells(|cells_update| {
      cells_update.update("f1", TestTextCell("hello world".to_string()));
    });
  });

  let cells = database_test.get_cells_for_field("v1", "f1");
  assert_eq!(cells[0].get_str_value("data").unwrap(), "hello world");
}

#[test]
fn update_empty_cell_for_field_test() {
  let database_test = create_database_with_default_data(1, "1");
  let cells = database_test.get_cells_for_field("v1", "f2");
  assert_eq!(cells.len(), 2);

  database_test.update_row(3, |row_update| {
    row_update.update_cells(|cells_update| {
      cells_update.update("f2", TestTextCell("hello world".to_string()));
    });
  });

  let cells = database_test.get_cells_for_field("v1", "f2");
  assert_eq!(cells.len(), 3);
  assert_eq!(cells[2].get_str_value("data").unwrap(), "hello world");
}
