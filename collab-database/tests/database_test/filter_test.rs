use crate::database_test::helper::{DatabaseTest, create_database_with_default_data};
use crate::helper::{FILTER_CONTENT, TestFieldType, TestFilter};

#[tokio::test]
async fn create_database_view_with_filter_test() {
  let database_test = create_database_with_two_filters().await;
  let filter_1 = database_test
    .get_filter::<TestFilter>("v1", "filter_1")
    .unwrap();
  assert_eq!(filter_1.content, "hello filter");

  let filter_2 = database_test
    .get_filter::<TestFilter>("v1", "filter_2")
    .unwrap();
  assert_eq!(filter_2.field_type, TestFieldType::Number);
}

#[tokio::test]
async fn insert_or_update_database_view_filter_test() {
  let mut database_test = create_database_with_two_filters().await;
  // Update
  database_test.update_filter("v1", "filter_1", |update| {
    update.insert(FILTER_CONTENT.into(), "Text filter".into());
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

#[tokio::test]
async fn insert_database_view_filter_to_filtering_field_id_test() {
  let mut database_test = create_database_with_two_filters().await;

  // Filter with id "filter_1" already filters based on "f1"
  database_test.insert_filter(
    "v1",
    TestFilter {
      id: "filter_3".to_string(),
      field_id: "f1".to_string(),
      field_type: Default::default(),
      condition: 0,
      content: "Another filter".to_string(),
    },
  );

  let filter_3 = database_test
    .get_filter::<TestFilter>("v1", "filter_3")
    .unwrap();
  assert_eq!(filter_3.content, "Another filter");
}

#[tokio::test]
async fn remove_database_view_filter_test() {
  let mut database_test = create_database_with_two_filters().await;
  database_test.remove_filter("v1", "filter_1");
  let filter_1 = database_test.get_filter::<TestFilter>("v1", "filter_1");
  assert!(filter_1.is_none());
}

async fn create_database_with_two_filters() -> DatabaseTest {
  let database_id = uuid::Uuid::new_v4();
  let mut database_test = create_database_with_default_data(1, &database_id.to_string()).await;
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
