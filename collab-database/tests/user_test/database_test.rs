use collab_database::database::gen_database_view_id;
use collab_database::rows::CreateRowParams;
use collab_database::views::{CreateDatabaseParams, CreateViewParams};

use crate::user_test::helper::{
  make_default_grid, random_uid, user_database_test_with_db, user_database_test_with_default_data,
  workspace_database_test,
};

#[tokio::test]
async fn create_database_test() {
  let uid = random_uid();
  let test = workspace_database_test(uid).await;
  let database = test
    .create_database(CreateDatabaseParams {
      database_id: "d1".to_string(),
      inline_view_id: "v1".to_string(),
      views: vec![CreateViewParams {
        database_id: "d1".to_string(),
        view_id: "v1".to_string(),
        ..Default::default()
      }],
      ..Default::default()
    })
    .unwrap();

  let views = database.lock().views.get_all_views();
  assert_eq!(views.len(), 1);
}

#[tokio::test]
async fn create_multiple_database_test() {
  let uid = random_uid();
  let test = workspace_database_test(uid).await;
  test
    .create_database(CreateDatabaseParams {
      database_id: "d1".to_string(),
      inline_view_id: "v1".to_string(),
      views: vec![CreateViewParams {
        database_id: "d1".to_string(),
        view_id: "v1".to_string(),
        ..Default::default()
      }],
      ..Default::default()
    })
    .unwrap();
  test
    .create_database(CreateDatabaseParams {
      database_id: "d2".to_string(),
      inline_view_id: "v2".to_string(),
      views: vec![CreateViewParams {
        database_id: "d2".to_string(),
        view_id: "v2".to_string(),
        ..Default::default()
      }],
      ..Default::default()
    })
    .unwrap();
  let all_databases = test.get_all_database_meta();
  assert_eq!(all_databases.len(), 2);
  assert_eq!(all_databases[0].database_id, "d1");
  assert_eq!(all_databases[1].database_id, "d2");
}

#[tokio::test]
async fn delete_database_test() {
  let uid = random_uid();
  let test = workspace_database_test(uid).await;
  test
    .create_database(CreateDatabaseParams {
      database_id: "d1".to_string(),
      inline_view_id: "v1".to_string(),
      views: vec![CreateViewParams {
        database_id: "d1".to_string(),
        view_id: "v1".to_string(),
        ..Default::default()
      }],
      ..Default::default()
    })
    .unwrap();
  test
    .create_database(CreateDatabaseParams {
      database_id: "d2".to_string(),
      inline_view_id: "v2".to_string(),
      views: vec![CreateViewParams {
        database_id: "d2".to_string(),
        view_id: "v2".to_string(),
        ..Default::default()
      }],
      ..Default::default()
    })
    .unwrap();
  test.delete_database("d1");

  let all_databases = test.get_all_database_meta();
  assert_eq!(all_databases[0].database_id, "d2");
}

#[tokio::test]
async fn duplicate_database_inline_view_test() {
  let uid = random_uid();
  let test = workspace_database_test(uid).await;
  let database = test
    .create_database(CreateDatabaseParams {
      database_id: "d1".to_string(),
      inline_view_id: "v1".to_string(),
      views: vec![CreateViewParams {
        database_id: "d1".to_string(),
        view_id: "v1".to_string(),
        ..Default::default()
      }],
      ..Default::default()
    })
    .unwrap();

  let duplicated_database = test.duplicate_database("v1").await.unwrap();
  let duplicated_view_id = duplicated_database.lock().get_inline_view_id();
  duplicated_database
    .lock()
    .create_row(CreateRowParams {
      id: 1.into(),
      ..Default::default()
    })
    .unwrap();

  assert_eq!(
    duplicated_database
      .lock()
      .get_rows_for_view(&duplicated_view_id)
      .len(),
    1
  );
  assert!(database.lock().get_rows_for_view("v1").is_empty());
}

#[tokio::test]
async fn duplicate_database_view_test() {
  let test = workspace_database_test(random_uid()).await;

  // create the database with inline view
  let database = test
    .create_database(CreateDatabaseParams {
      database_id: "d1".to_string(),
      inline_view_id: "v1".to_string(),
      views: vec![CreateViewParams {
        database_id: "d1".to_string(),
        view_id: "v1".to_string(),
        ..Default::default()
      }],
      ..Default::default()
    })
    .unwrap();

  test
    .create_database_linked_view(CreateViewParams {
      database_id: "d1".to_string(),
      view_id: "v2".to_string(),
      ..Default::default()
    })
    .await
    .unwrap();

  // Duplicate the linked view.
  let duplicated_view = database.lock().duplicate_linked_view("v2").unwrap();
  database
    .lock()
    .create_row(CreateRowParams {
      id: 1.into(),
      ..Default::default()
    })
    .unwrap();

  // Duplicated database should have the same rows as the original database
  assert_eq!(
    database.lock().get_rows_for_view(&duplicated_view.id).len(),
    1
  );
  assert_eq!(database.lock().get_rows_for_view("v1").len(), 1);
}

#[tokio::test]
async fn delete_database_linked_view_test() {
  let test = workspace_database_test(random_uid()).await;
  let database = test
    .create_database(CreateDatabaseParams {
      database_id: "d1".to_string(),
      inline_view_id: "v1".to_string(),
      views: vec![CreateViewParams {
        database_id: "d1".to_string(),
        view_id: "v1".to_string(),
        ..Default::default()
      }],
      ..Default::default()
    })
    .unwrap();

  database
    .lock()
    .create_linked_view(CreateViewParams {
      database_id: "d1".to_string(),
      view_id: "v2".to_string(),
      ..Default::default()
    })
    .unwrap();

  let views = database.lock().views.get_all_views();
  assert_eq!(views.len(), 2);

  database.lock().delete_view("v2");

  let views = database.lock().views.get_all_views();
  assert_eq!(views.len(), 1);

  database.lock().delete_view("v1");

  let views = database.lock().views.get_all_views();
  assert_eq!(views.len(), 0);
}

#[tokio::test]
async fn delete_database_inline_view_test() {
  let test = workspace_database_test(random_uid()).await;
  let database = test
    .create_database(CreateDatabaseParams {
      database_id: "d1".to_string(),
      inline_view_id: "v1".to_string(),
      views: vec![CreateViewParams {
        database_id: "d1".to_string(),
        view_id: "v1".to_string(),
        ..Default::default()
      }],
      ..Default::default()
    })
    .unwrap();

  for i in 2..5 {
    database
      .lock()
      .create_linked_view(CreateViewParams {
        database_id: "d1".to_string(),
        view_id: format!("v{}", i),
        ..Default::default()
      })
      .unwrap();
  }

  // there should be 4 views: inline-view v1 and created linked-views v2, v3 and v4.
  let views = database.lock().views.get_all_views();
  assert_eq!(views.len(), 4);

  // After deleting the inline view, all linked views will be removed
  test.delete_view("d1", "v1").await;
  let views = database.lock().views.get_all_views();
  assert_eq!(views.len(), 0);
}

#[tokio::test]
async fn duplicate_database_data_test() {
  let test = user_database_test_with_default_data(random_uid()).await;
  let original = test.get_database_with_view_id("v1").await.unwrap();
  let duplicate = test.duplicate_database("v1").await.unwrap();

  let duplicated_view_id = &duplicate.lock().get_all_database_views_meta()[0].id;

  // compare rows
  let original_rows = original.lock().get_rows_for_view("v1");
  let duplicate_rows = duplicate.lock().get_rows_for_view(duplicated_view_id);
  assert_eq!(original_rows.len(), duplicate_rows.len());
  for (index, row) in original_rows.iter().enumerate() {
    assert_eq!(row.visibility, duplicate_rows[index].visibility);
    assert_eq!(row.cells, duplicate_rows[index].cells);
    assert_eq!(row.height, duplicate_rows[index].height);
  }

  // compare views
  let original_views = original.lock().views.get_all_views();
  let duplicated_views = duplicate.lock().views.get_all_views();
  assert_eq!(original_views.len(), duplicated_views.len());

  // compare inline view
  let original_inline_view_id = original.lock().get_inline_view_id();
  let original_inline_view = original.lock().get_view(&original_inline_view_id).unwrap();
  let duplicated_inline_view_id = duplicate.lock().get_inline_view_id();
  let duplicated_inline_view = duplicate
    .lock()
    .get_view(&duplicated_inline_view_id)
    .unwrap();
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

#[tokio::test]
async fn get_database_by_view_id_test() {
  let test = workspace_database_test(random_uid()).await;
  let _database = test
    .create_database(CreateDatabaseParams {
      database_id: "d1".to_string(),
      inline_view_id: "v1".to_string(),
      views: vec![CreateViewParams {
        database_id: "d1".to_string(),
        view_id: "v1".to_string(),
        ..Default::default()
      }],
      ..Default::default()
    })
    .unwrap();

  test
    .create_database_linked_view(CreateViewParams {
      database_id: "d1".to_string(),
      view_id: "v2".to_string(),
      ..Default::default()
    })
    .await
    .unwrap();

  let database = test.get_database_with_view_id("v2").await;
  assert!(database.is_some());
}

#[tokio::test]
async fn reopen_database_test() {
  let uid = random_uid();
  let test = workspace_database_test(uid).await;
  let view_id = gen_database_view_id();
  let params = make_default_grid(&view_id, "first view");

  // create the database with inline view
  let _database = test.create_database(params).unwrap();
  let db = test.collab_db.clone();
  drop(test);

  let test = user_database_test_with_db(uid, db).await;
  let database = test.get_database_with_view_id(&view_id).await;
  let _ = database.unwrap().lock().to_json_value();
}
