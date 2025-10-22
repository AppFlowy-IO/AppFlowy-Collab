use crate::database_test::helper::{
  DatabaseTest, TEST_VIEW_ID_V1, create_database_with_default_data,
};
use crate::helper::{SortCondition, TestSort};
use collab::database::entity::CreateViewParams;
use collab::database::views::DatabaseLayout;

#[tokio::test]
async fn create_database_view_with_sort_test() {
  let database_test = create_database_with_two_sorts().await;
  let sorts = database_test.get_all_sorts::<TestSort>(TEST_VIEW_ID_V1);
  assert_eq!(sorts.len(), 2);
  assert_eq!(sorts[0].condition, SortCondition::Ascending);
  assert_eq!(sorts[1].condition, SortCondition::Descending);
}

#[tokio::test]
async fn get_database_view_sort_test() {
  let mut database_test = create_database_with_two_sorts().await;

  database_test.insert_sort(
    TEST_VIEW_ID_V1,
    TestSort {
      id: "s3".to_string(),
      field_id: "f1".to_string(),
      field_type: 0,
      condition: Default::default(),
    },
  );

  let sort = database_test.get_sort::<TestSort>(TEST_VIEW_ID_V1, "s3");
  assert!(sort.is_some());
}

#[tokio::test]
async fn update_database_view_sort_test() {
  let mut database_test = create_database_with_two_sorts().await;
  let sort_1 = TestSort {
    id: "s1".to_string(),
    field_id: "f1".to_string(),
    field_type: Default::default(),
    condition: SortCondition::Ascending,
  };
  database_test.insert_sort(TEST_VIEW_ID_V1, sort_1);

  let sorts = database_test
    .get_view(TEST_VIEW_ID_V1)
    .unwrap()
    .sorts
    .into_iter()
    .map(|value| TestSort::try_from(value).unwrap())
    .collect::<Vec<TestSort>>();
  assert_eq!(sorts.len(), 2);
  assert_eq!(sorts[0].condition, SortCondition::Ascending);
}

#[tokio::test]
async fn remove_all_database_view_sort_test() {
  let mut database_test = create_database_with_two_sorts().await;
  database_test.remove_all_sorts(TEST_VIEW_ID_V1);

  let view = database_test.get_view(TEST_VIEW_ID_V1).unwrap();
  assert!(view.sorts.is_empty());
}

#[tokio::test]
async fn remove_database_view_sort_test() {
  let mut database_test = create_database_with_two_sorts().await;
  database_test.remove_sort(TEST_VIEW_ID_V1, "s1");

  let view = database_test.get_view(TEST_VIEW_ID_V1).unwrap();
  assert_eq!(view.sorts.len(), 1);
}

#[tokio::test]
async fn reorder_database_view_sort_test() {
  let mut database_test = create_database_with_two_sorts().await;
  database_test.move_sort(TEST_VIEW_ID_V1, "s2", "s1");

  let sorts = database_test
    .get_view(TEST_VIEW_ID_V1)
    .unwrap()
    .sorts
    .into_iter()
    .map(|value| TestSort::try_from(value).unwrap())
    .collect::<Vec<TestSort>>();

  assert_eq!(sorts.len(), 2);
  assert_eq!(sorts[0].id, "s2");
  assert_eq!(sorts[1].id, "s1");
}

async fn create_database_with_two_sorts() -> DatabaseTest {
  let database_id = uuid::Uuid::new_v4();
  let mut database_test = create_database_with_default_data(1, &database_id.to_string()).await;
  let sort_1 = TestSort {
    id: "s1".to_string(),
    field_id: "f1".to_string(),
    field_type: Default::default(),
    condition: SortCondition::Ascending,
  };
  let sort_2 = TestSort {
    id: "s2".to_string(),
    field_id: "f2".to_string(),
    field_type: Default::default(),
    condition: SortCondition::Descending,
  };

  let params = CreateViewParams {
    database_id,
    view_id: uuid::Uuid::parse_str(TEST_VIEW_ID_V1).unwrap(),
    sorts: vec![sort_1.into(), sort_2.into()],
    layout: DatabaseLayout::Grid,
    ..Default::default()
  };
  database_test.create_linked_view(params).unwrap();
  database_test
}
