use crate::user_test::helper::{
  make_default_grid, random_uid, user_database_test, user_database_test_with_db,
  user_database_test_with_default_data,
};
use collab_database::block::CreateRowParams;
use collab_database::views::{CreateDatabaseParams, CreateViewParams};

#[test]
fn create_database_test() {
  let uid = random_uid();
  let _ = user_database_test(uid);
}

#[test]
fn create_multiple_database_test() {
  let uid = random_uid();
  let test = user_database_test(uid);
  test
    .create_database(CreateDatabaseParams {
      database_id: "d1".to_string(),
      view_id: "v1".to_string(),
      ..Default::default()
    })
    .unwrap();
  test
    .create_database(CreateDatabaseParams {
      database_id: "d2".to_string(),
      view_id: "v1".to_string(),
      ..Default::default()
    })
    .unwrap();
  let all_databases = test.get_all_databases();
  assert_eq!(all_databases.len(), 2);
  assert_eq!(all_databases[0].database_id, "d1");
  assert_eq!(all_databases[1].database_id, "d2");
}

#[test]
fn delete_database_test() {
  let uid = random_uid();
  let test = user_database_test(uid);
  test
    .create_database(CreateDatabaseParams {
      database_id: "d1".to_string(),
      view_id: "v1".to_string(),
      ..Default::default()
    })
    .unwrap();
  test
    .create_database(CreateDatabaseParams {
      database_id: "d2".to_string(),
      view_id: "v1".to_string(),
      ..Default::default()
    })
    .unwrap();
  test.delete_database("d1");

  let all_databases = test.get_all_databases();
  assert_eq!(all_databases[0].database_id, "d2");
}

#[test]
fn duplicate_database_inline_view_test() {
  let uid = random_uid();
  let test = user_database_test(uid);
  let database = test
    .create_database(CreateDatabaseParams {
      database_id: "d1".to_string(),
      view_id: "v1".to_string(),
      ..Default::default()
    })
    .unwrap();

  let duplicated_database = test.duplicate_database("v1").unwrap();
  let duplicated_view_id = duplicated_database.get_inline_view_id();
  duplicated_database.create_row(CreateRowParams {
    id: 1.into(),
    ..Default::default()
  });

  assert_eq!(
    duplicated_database
      .get_rows_for_view(&duplicated_view_id)
      .len(),
    1
  );
  assert!(database.get_rows_for_view("v1").is_empty());
}

#[test]
fn duplicate_database_view_test() {
  let test = user_database_test(random_uid());

  // create the database with inline view
  let database = test
    .create_database(CreateDatabaseParams {
      database_id: "d1".to_string(),
      view_id: "v1".to_string(),
      ..Default::default()
    })
    .unwrap();

  test.create_database_view(CreateViewParams {
    database_id: "d1".to_string(),
    view_id: "v2".to_string(),
    ..Default::default()
  });

  // Duplicate the linked view.
  let duplicated_view = database.duplicate_linked_view("v2").unwrap();
  database.create_row(CreateRowParams {
    id: 1.into(),
    ..Default::default()
  });

  // Duplicated database should have the same rows as the original database
  assert_eq!(database.get_rows_for_view(&duplicated_view.id).len(), 1);
  assert_eq!(database.get_rows_for_view("v1").len(), 1);
}

#[test]
fn delete_database_inline_view_test() {
  let test = user_database_test(random_uid());
  let database = test
    .create_database(CreateDatabaseParams {
      database_id: "d1".to_string(),
      view_id: "v1".to_string(),
      ..Default::default()
    })
    .unwrap();

  for i in 2..5 {
    database.create_linked_view(CreateViewParams {
      view_id: format!("v{}", i),
      ..Default::default()
    });
  }

  let views = database.views.get_all_views();
  assert_eq!(views.len(), 4);

  // After deleting the inline view, all linked views will be removed
  test.delete_view("d1", "v1");
  let views = database.views.get_all_views();
  assert_eq!(views.len(), 0);
}

#[test]
fn duplicate_database_data_test() {
  let test = user_database_test_with_default_data(random_uid());
  let original = test.get_database_with_view_id("v1").unwrap();
  let duplicated_data = test.get_database_duplicated_data("v1").unwrap();
  let duplicate = test
    .create_database_with_duplicated_data(duplicated_data)
    .unwrap();

  let duplicated_view_id = &duplicate.get_all_views_description()[0].id;

  // compare rows
  let original_rows = original.get_rows_for_view("v1");
  let duplicate_rows = duplicate.get_rows_for_view(duplicated_view_id);
  assert_eq!(original_rows.len(), duplicate_rows.len());
  for (index, row) in original_rows.iter().enumerate() {
    assert_eq!(row.visibility, duplicate_rows[index].visibility);
    assert_eq!(row.cells, duplicate_rows[index].cells);
    assert_eq!(row.height, duplicate_rows[index].height);
  }

  // compare views
  let original_views = original.views.get_all_views();
  let duplicated_views = duplicate.views.get_all_views();
  assert_eq!(original_views.len(), duplicated_views.len());

  // compare inline view
  let original_inline_view_id = original.get_inline_view_id();
  let original_inline_view = original.get_view(&original_inline_view_id).unwrap();
  let duplicated_inline_view_id = duplicate.get_inline_view_id();
  let duplicated_inline_view = duplicate.get_view(&duplicated_inline_view_id).unwrap();
  assert_eq!(
    original_inline_view.row_orders.len(),
    duplicated_inline_view.row_orders.len()
  );
  assert_eq!(
    original_inline_view.field_orders.len(),
    duplicated_inline_view.field_orders.len()
  );

  // compare field orders
  assert_eq!(duplicated_inline_view.field_orders[0].id, "f1");
  assert_eq!(duplicated_inline_view.field_orders[1].id, "f2");
  assert_eq!(duplicated_inline_view.field_orders[2].id, "f3");
}

#[test]
fn get_database_by_view_id_test() {
  let test = user_database_test(random_uid());
  let _database = test
    .create_database(CreateDatabaseParams {
      database_id: "d1".to_string(),
      view_id: "v1".to_string(),
      ..Default::default()
    })
    .unwrap();

  test.create_database_view(CreateViewParams {
    database_id: "d1".to_string(),
    view_id: "v2".to_string(),
    ..Default::default()
  });

  let database = test.get_database_with_view_id("v2");
  assert!(database.is_some());
}

#[test]
fn reopen_database_test() {
  let uid = random_uid();
  let test = user_database_test(uid);
  let params = make_default_grid("v1", "first view");
  let _database = test.create_database(params).unwrap();
  // let expect_json = database.to_json_value();
  let db = test.db.clone();
  drop(test);

  let test = user_database_test_with_db(uid, db);
  let database = test.get_database_with_view_id("v1");
  let _ = database.unwrap().to_json_value();
}
