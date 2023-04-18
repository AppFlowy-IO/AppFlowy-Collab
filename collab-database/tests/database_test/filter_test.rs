use crate::database_test::helper::{create_database_with_default_data, DatabaseTest};
use crate::helper::{TestFieldType, TestFilter, FILTER_CONTENT};
use collab::core::any_map::AnyMapExtension;

#[test]
fn create_database_view_with_filter_test() {
  let database_test = create_database_with_two_filters();
  let filter_1 = database_test
    .get_filter::<TestFilter>("v1", "filter_1")
    .unwrap();
  assert_eq!(filter_1.content, "hello filter");

  let filter_2 = database_test
    .get_filter::<TestFilter>("v1", "filter_2")
    .unwrap();
  assert_eq!(filter_2.field_type, TestFieldType::Number);
}

#[test]
fn insert_or_update_database_view_filter_test() {
  let database_test = create_database_with_two_filters();
  // Update
  database_test.update_filter("v1", "filter_1", |update| {
    update.insert_str_value(FILTER_CONTENT, "Text filter".to_string());
  });

  let filter_1 = database_test
    .get_filter::<TestFilter>("v1", "filter_1")
    .unwrap();
  assert_eq!(filter_1.content, "Text filter");

  // Insert
  database_test.insert_filter(
    "v1",
    TestFilter {
      id: "filter_1".to_string(),
      field_id: "".to_string(),
      field_type: Default::default(),
      condition: 0,
      content: "Override the existing filter".to_string(),
    },
  );

  let filter_1 = database_test
    .get_filter::<TestFilter>("v1", "filter_1")
    .unwrap();
  assert_eq!(filter_1.content, "Override the existing filter");
}

#[test]
fn get_database_view_filter_by_field_id_test() {
  let database_test = create_database_with_two_filters();
  let filter_1 = database_test
    .get_filter_by_field_id::<TestFilter>("v1", "f1")
    .unwrap();
  assert_eq!(filter_1.content, "hello filter");
}

#[test]
fn insert_database_view_filter_with_occupied_field_id_test() {
  let database_test = create_database_with_two_filters();

  // The field id "f1" is already occupied by existing filter. So this filter
  // will be ignored
  database_test.insert_filter(
    "v1",
    TestFilter {
      id: "filter_3".to_string(),
      field_id: "f1".to_string(),
      field_type: Default::default(),
      condition: 0,
      content: "Override the existing filter".to_string(),
    },
  );

  let filter_1 = database_test
    .get_filter_by_field_id::<TestFilter>("v1", "f1")
    .unwrap();
  assert_eq!(filter_1.content, "hello filter");
}

#[test]
fn remove_database_view_filter_test() {
  let database_test = create_database_with_two_filters();
  database_test.remove_filter("v1", "filter_1");
  let filter_1 = database_test.get_filter::<TestFilter>("v1", "filter_1");
  assert!(filter_1.is_none());
}

fn create_database_with_two_filters() -> DatabaseTest {
  let database_test = create_database_with_default_data(1, "1");
  let filter_1 = TestFilter {
    id: "filter_1".to_string(),
    field_id: "f1".to_string(),
    field_type: TestFieldType::RichText,
    condition: 0,
    content: "hello filter".to_string(),
  };
  let filter_2 = TestFilter {
    id: "filter_2".to_string(),
    field_id: "f2".to_string(),
    field_type: TestFieldType::Number,
    condition: 0,
    content: "".to_string(),
  };

  database_test.insert_filter("v1", filter_1);
  database_test.insert_filter("v1", filter_2);

  database_test
}
