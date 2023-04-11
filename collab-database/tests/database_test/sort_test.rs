use crate::helper::{create_database_with_default_data, DatabaseTest, SortCondition, TestSort};

use collab_database::views::{CreateViewParams, DatabaseLayout};

#[test]
fn create_database_view_with_sort_test() {
  let database_test = create_database_with_two_sorts();
  let view = database_test.views.get_view("v1").unwrap();
  assert_eq!(view.sorts.len(), 2);
}

#[test]
fn update_database_view_sort_test() {
  let database_test = create_database_with_two_sorts();
  let sort_1 = TestSort {
    id: "s1".to_string(),
    field_id: "f1".to_string(),
    field_type: Default::default(),
    condition: SortCondition::Ascending,
  };
  database_test.insert_sort("v1", sort_1);

  let sorts = database_test
    .views
    .get_view("v1")
    .unwrap()
    .sorts
    .into_iter()
    .map(|value| TestSort::try_from(value).unwrap())
    .collect::<Vec<TestSort>>();
  assert_eq!(sorts.len(), 2);
  assert_eq!(sorts[0].condition, SortCondition::Ascending);
}

#[test]
fn remove_all_database_view_sort_test() {
  let database_test = create_database_with_two_sorts();
  database_test.remove_all_sorts("v1");

  let view = database_test.views.get_view("v1").unwrap();
  assert!(view.sorts.is_empty());
}
#[test]
fn remove_database_view_sort_test() {
  let database_test = create_database_with_two_sorts();
  database_test.remove_sort("v1", "s1");

  let view = database_test.views.get_view("v1").unwrap();
  assert_eq!(view.sorts.len(), 1);
}

fn create_database_with_two_sorts() -> DatabaseTest {
  let database_test = create_database_with_default_data(1, "1");
  let sort_1 = TestSort {
    id: "s1".to_string(),
    field_id: "f1".to_string(),
    field_type: Default::default(),
    condition: SortCondition::Descending,
  };
  let sort_2 = TestSort {
    id: "s2".to_string(),
    field_id: "f2".to_string(),
    field_type: Default::default(),
    condition: SortCondition::Descending,
  };

  let params = CreateViewParams {
    view_id: "v1".to_string(),
    sorts: vec![sort_1.into(), sort_2.into()],
    layout: DatabaseLayout::Grid,
    ..Default::default()
  };
  database_test.create_view(params);
  database_test
}
