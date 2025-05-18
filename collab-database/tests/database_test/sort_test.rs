use crate::database_test::helper::{DatabaseTest, create_database_with_default_data};
use crate::helper::{SortCondition, TestSort};
use collab_database::entity::CreateViewParams;
use collab_database::views::DatabaseLayout;

#[tokio::test]
async fn create_database_view_with_sort_test() {
  let database_test = create_database_with_two_sorts().await;
  let sorts = database_test.get_all_sorts::<TestSort>("v1");
  assert_eq!(sorts.len(), 2);
  assert_eq!(sorts[0].condition, SortCondition::Ascending);
  assert_eq!(sorts[1].condition, SortCondition::Descending);
}

#[tokio::test]
async fn get_database_view_sort_test() {
  let mut database_test = create_database_with_two_sorts().await;

  database_test.insert_sort(
    "v1",
    TestSort {
      id: "s3".to_string(),
      field_id: "f1".to_string(),
      field_type: 0,
      condition: Default::default(),
    },
  );

  let sort = database_test.get_sort::<TestSort>("v1", "s3");
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
  database_test.insert_sort("v1", sort_1);

  let sorts = database_test
    .get_view("v1")
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
  database_test.remove_all_sorts("v1");

  let view = database_test.get_view("v1").unwrap();
  assert!(view.sorts.is_empty());
}

#[tokio::test]
async fn remove_database_view_sort_test() {
  let mut database_test = create_database_with_two_sorts().await;
  database_test.remove_sort("v1", "s1");

  let view = database_test.get_view("v1").unwrap();
  assert_eq!(view.sorts.len(), 1);
}

#[tokio::test]
async fn reorder_database_view_sort_test() {
  let mut database_test = create_database_with_two_sorts().await;
  database_test.move_sort("v1", "s2", "s1");

  let sorts = database_test
    .get_view("v1")
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
    database_id: database_id.to_string(),
    view_id: "v1".to_string(),
    sorts: vec![sort_1.into(), sort_2.into()],
    layout: DatabaseLayout::Grid,
    ..Default::default()
  };
  database_test.create_linked_view(params).unwrap();
  database_test
}
