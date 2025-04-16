use crate::user_test::helper::{
  make_default_grid, random_uid, user_database_test_with_db, user_database_test_with_default_data,
  workspace_database_test,
};
use collab_database::database::gen_database_view_id;
use collab_database::entity::{CreateDatabaseParams, CreateViewParams, FileUploadType};
use collab_database::rows::{CoverType, CreateRowParams, Row, RowCover};
use futures::StreamExt;
use uuid::Uuid;

#[tokio::test]
async fn create_database_test() {
  let uid = random_uid();
  let database_id = Uuid::new_v4();
  let mut test = workspace_database_test(uid).await;
  let database = test
    .create_database(CreateDatabaseParams {
      database_id: database_id.to_string(),
      views: vec![CreateViewParams {
        database_id: database_id.to_string(),
        view_id: "v1".to_string(),
        ..Default::default()
      }],
      ..Default::default()
    })
    .await
    .unwrap();

  let db = database.read().await;
  let view_id = db.get_first_database_view_id().unwrap();
  let meta = test.get_database_meta(&database_id.to_string()).unwrap();

  // Inline view id should not appear in the database's linked views.
  assert!(!meta.linked_views.contains(&view_id));

  let views = db.get_all_views();
  assert_eq!(views.len(), 1);
}

#[tokio::test]
async fn create_multiple_database_test() {
  let uid = random_uid();
  let mut test = workspace_database_test(uid).await;
  let database_id_1 = Uuid::new_v4();
  test
    .create_database(CreateDatabaseParams {
      database_id: database_id_1.to_string(),
      views: vec![CreateViewParams {
        database_id: database_id_1.to_string(),
        view_id: "v1".to_string(),
        ..Default::default()
      }],
      ..Default::default()
    })
    .await
    .unwrap();

  let database_id_2 = Uuid::new_v4();
  test
    .create_database(CreateDatabaseParams {
      database_id: database_id_2.to_string(),
      views: vec![CreateViewParams {
        database_id: database_id_2.to_string(),
        view_id: "v2".to_string(),
        ..Default::default()
      }],
      ..Default::default()
    })
    .await
    .unwrap();
  let all_databases = test.get_all_database_meta();
  assert_eq!(all_databases.len(), 2);
  assert_eq!(all_databases[0].database_id, database_id_1.to_string());
  assert_eq!(all_databases[1].database_id, database_id_2.to_string());
}

#[tokio::test]
async fn delete_database_test() {
  let uid = random_uid();
  let database_id_1 = Uuid::new_v4();
  let mut test = workspace_database_test(uid).await;
  test
    .create_database(CreateDatabaseParams {
      database_id: database_id_1.to_string(),
      views: vec![CreateViewParams {
        database_id: database_id_1.to_string(),
        view_id: "v1".to_string(),
        ..Default::default()
      }],
      ..Default::default()
    })
    .await
    .unwrap();
  let database_id_2 = Uuid::new_v4();
  test
    .create_database(CreateDatabaseParams {
      database_id: database_id_2.to_string(),
      views: vec![CreateViewParams {
        database_id: database_id_2.to_string(),
        view_id: "v2".to_string(),
        ..Default::default()
      }],
      ..Default::default()
    })
    .await
    .unwrap();
  test.delete_database(&database_id_1.to_string());

  let all_databases = test.get_all_database_meta();
  assert_eq!(all_databases[0].database_id, database_id_2.to_string());
}

#[tokio::test]
async fn duplicate_database_inline_view_test() {
  let uid = random_uid();
  let mut test = workspace_database_test(uid).await;
  let database_id = Uuid::new_v4();
  let database = test
    .create_database(CreateDatabaseParams {
      database_id: database_id.to_string(),
      views: vec![CreateViewParams {
        database_id: database_id.to_string(),
        view_id: "v1".to_string(),
        ..Default::default()
      }],
      ..Default::default()
    })
    .await
    .unwrap();

  let duplicated_database = test.duplicate_database("v1", "v1_1").await.unwrap();
  let mut db = duplicated_database.write().await;
  let duplicated_view_id = db.get_first_database_view_id().unwrap();
  db.create_row(CreateRowParams::new(1, database_id.to_string()))
    .await
    .unwrap();

  assert_eq!(
    db.get_rows_for_view(&duplicated_view_id, 20, None)
      .await
      .count()
      .await,
    1
  );
  assert_eq!(
    database
      .read()
      .await
      .get_rows_for_view("v1", 10, None)
      .await
      .count()
      .await,
    0
  );
}

#[tokio::test]
async fn duplicate_database_view_test() {
  let mut test = workspace_database_test(random_uid()).await;

  // create the database with inline view
  let database_id = Uuid::new_v4();
  let database = test
    .create_database(CreateDatabaseParams {
      database_id: database_id.to_string(),
      views: vec![CreateViewParams {
        database_id: database_id.to_string(),
        view_id: "v1".to_string(),
        ..Default::default()
      }],
      ..Default::default()
    })
    .await
    .unwrap();

  test
    .create_database_linked_view(CreateViewParams {
      database_id: database_id.to_string(),
      view_id: "v2".to_string(),
      ..Default::default()
    })
    .await
    .unwrap();

  // Duplicate the linked view.
  let mut db = database.write().await;
  let duplicated_view = db.duplicate_linked_view("v2").unwrap();
  db.create_row(CreateRowParams::new(1, database_id.to_string()))
    .await
    .unwrap();

  // Duplicated database should have the same rows as the original database
  assert_eq!(
    db.get_rows_for_view(&duplicated_view.id, 10, None)
      .await
      .count()
      .await,
    1
  );
  assert_eq!(db.get_rows_for_view("v1", 10, None).await.count().await, 1);
}

#[tokio::test]
async fn delete_database_linked_view_test() {
  let mut test = workspace_database_test(random_uid()).await;
  let database_id = Uuid::new_v4();
  let database = test
    .create_database(CreateDatabaseParams {
      database_id: database_id.to_string(),
      views: vec![CreateViewParams {
        database_id: database_id.to_string(),
        view_id: "v1".to_string(),
        ..Default::default()
      }],
      ..Default::default()
    })
    .await
    .unwrap();

  let mut db = database.write().await;
  db.create_linked_view(CreateViewParams {
    database_id: database_id.to_string(),
    view_id: "v2".to_string(),
    ..Default::default()
  })
  .unwrap();

  let views = db.get_all_views();
  assert_eq!(views.len(), 2);

  db.delete_view("v2");

  let views = db.get_all_views();
  assert_eq!(views.len(), 1);

  db.delete_view("v1");

  let views = db.get_all_views();
  assert_eq!(views.len(), 0);
}

#[tokio::test]
async fn delete_database_inline_view_test() {
  let mut test = workspace_database_test(random_uid()).await;
  let database_id = Uuid::new_v4();
  let database = test
    .create_database(CreateDatabaseParams {
      database_id: database_id.to_string(),
      views: vec![CreateViewParams {
        database_id: database_id.to_string(),
        view_id: "v1".to_string(),
        ..Default::default()
      }],
      ..Default::default()
    })
    .await
    .unwrap();

  let mut db = database.write().await;
  for i in 2..5 {
    db.create_linked_view(CreateViewParams {
      database_id: database_id.to_string(),
      view_id: format!("v{}", i),
      ..Default::default()
    })
    .unwrap();
  }

  // there should be 4 views: v1, v2, v3 and v4.
  let views = db.get_all_views();
  assert_eq!(views.len(), 4);
  drop(db);

  test.delete_view(&database_id.to_string(), "v1").await;
  let views = database.read().await.get_all_views();
  assert_eq!(views.len(), 3);
}

#[tokio::test]
async fn duplicate_database_data_test() {
  let mut test = user_database_test_with_default_data(random_uid()).await;
  let original = test.get_database_with_view_id("v1").await.unwrap();
  let duplicate = test.duplicate_database("v1", "v1_1").await.unwrap();
  let original = original.read().await;
  let duplicate = duplicate.read().await;

  let duplicated_view_id = &duplicate.get_all_database_views_meta()[0].id;
  assert_eq!(duplicated_view_id, "v1_1");

  // compare rows
  let original_rows: Vec<Row> = original
    .get_rows_for_view("v1", 10, None)
    .await
    .filter_map(|result| async { result.ok() })
    .collect()
    .await;

  let duplicate_rows: Vec<Row> = duplicate
    .get_rows_for_view(duplicated_view_id, 10, None)
    .await
    .filter_map(|result| async { result.ok() })
    .collect()
    .await;
  assert_eq!(original_rows.len(), duplicate_rows.len());
  for (index, row) in original_rows.iter().enumerate() {
    assert_eq!(row.visibility, duplicate_rows[index].visibility);
    assert_eq!(row.cells, duplicate_rows[index].cells);
    assert_eq!(row.height, duplicate_rows[index].height);
  }

  // compare views
  let original_views = original.get_all_views();
  let duplicated_views = duplicate.get_all_views();
  assert_eq!(original_views.len(), duplicated_views.len());

  // compare inline view
  let original_view_id = original.get_first_database_view_id().unwrap();
  let original_view = original.get_view(&original_view_id).unwrap();
  let duplicated_view_id = duplicate.get_first_database_view_id().unwrap();
  let duplicated_view = duplicate.get_view(&duplicated_view_id).unwrap();
  assert_eq!(
    original_view.row_orders.len(),
    duplicated_view.row_orders.len()
  );
  assert_eq!(
    original_view.field_orders.len(),
    duplicated_view.field_orders.len()
  );

  // compare field orders
  assert_eq!(duplicated_view.field_orders[0].id, "f1");
  assert_eq!(duplicated_view.field_orders[1].id, "f2");
  assert_eq!(duplicated_view.field_orders[2].id, "f3");
}

#[tokio::test]
async fn get_database_by_view_id_test() {
  let mut test = workspace_database_test(random_uid()).await;
  let database_id = Uuid::new_v4();
  let _database = test
    .create_database(CreateDatabaseParams {
      database_id: database_id.to_string(),
      views: vec![CreateViewParams {
        database_id: database_id.to_string(),
        view_id: "v1".to_string(),
        ..Default::default()
      }],
      ..Default::default()
    })
    .await
    .unwrap();

  test
    .create_database_linked_view(CreateViewParams {
      database_id: database_id.to_string(),
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
  let mut test = workspace_database_test(uid).await;
  let view_id = gen_database_view_id();
  let params = make_default_grid(&view_id, "first view");

  // create the database with inline view
  let database = test.create_database(params).await.unwrap();
  let row_orders = database.read().await.get_all_row_orders().await;
  for (index, row_order) in row_orders.into_iter().enumerate() {
    let cover = RowCover {
      data: format!("cover-{}", index),
      upload_type: FileUploadType::LocalFile,
      cover_type: CoverType::FileCover,
    };

    database
      .write()
      .await
      .update_row_meta(&row_order.id, |updater| {
        updater
          .insert_icon(&format!("icon-{}", index))
          .insert_cover(&cover);
      })
      .await;

    let row = database
      .read()
      .await
      .get_or_init_database_row(&row_order.id)
      .await
      .unwrap();
    let json = row.read().await.collab.to_json_value();
    assert!(json.get("meta").is_some());
  }

  let db = test.collab_db.clone();
  let workspace_id = test.workspace_id.clone();
  drop(test);

  let test = user_database_test_with_db(uid, &workspace_id, db).await;
  let database = test.get_database_with_view_id(&view_id).await.unwrap();
  let row_orders = database.read().await.get_all_row_orders().await;
  for (index, row_order) in row_orders.into_iter().enumerate() {
    let row_meta = database
      .read()
      .await
      .get_or_init_database_row(&row_order.id)
      .await
      .unwrap()
      .read()
      .await
      .get_row_meta()
      .unwrap();

    assert_eq!(row_meta.icon_url, Some(format!("icon-{}", index)));

    let cover = row_meta.cover.unwrap();
    assert_eq!(cover.data, format!("cover-{}", index));
  }
  let _ = database.read().await.to_json_value().await;
}
