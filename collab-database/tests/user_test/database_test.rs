use crate::database_test::helper::create_database_with_params;
use collab_database::entity::{CreateDatabaseParams, CreateViewParams};
use collab_database::rows::CreateRowParams;
use uuid::Uuid;

#[tokio::test]
async fn create_database_test() {
  let database_id = Uuid::new_v4();
  let view_id = "v1".to_string();
  let database = create_database_with_params(CreateDatabaseParams {
    database_id: database_id.to_string(),
    views: vec![CreateViewParams {
      database_id: database_id.to_string(),
      view_id: view_id.clone(),
      ..Default::default()
    }],
    ..Default::default()
  })
  .await;

  // Inline view id should not appear in the database's linked views.
  let non_inline_views = database.get_all_views();
  assert_eq!(non_inline_views.len(), 1);
  assert_eq!(non_inline_views[0].id, view_id);
}

//
// #[tokio::test]
// async fn delete_database_test() {
//   let uid = random_uid();
//   let database_id_1 = Uuid::new_v4();
//   let mut test = workspace_database_test(uid).await;
//   test
//     .create_database(CreateDatabaseParams {
//       database_id: database_id_1.to_string(),
//       views: vec![CreateViewParams {
//         database_id: database_id_1.to_string(),
//         view_id: "v1".to_string(),
//         ..Default::default()
//       }],
//       ..Default::default()
//     })
//     .await
//     .unwrap();
//   let database_id_2 = Uuid::new_v4();
//   test
//     .create_database(CreateDatabaseParams {
//       database_id: database_id_2.to_string(),
//       views: vec![CreateViewParams {
//         database_id: database_id_2.to_string(),
//         view_id: "v2".to_string(),
//         ..Default::default()
//       }],
//       ..Default::default()
//     })
//     .await
//     .unwrap();
//   test.delete_database(&database_id_1.to_string());
//
//   let all_databases = test.get_all_database_meta();
//   assert_eq!(all_databases[0].database_id, database_id_2.to_string());
// }
//
// #[tokio::test]
// async fn duplicate_database_inline_view_test() {
//   let uid = random_uid();
//   let mut test = workspace_database_test(uid).await;
//   let database_id = Uuid::new_v4();
//   let database = test
//     .create_database(CreateDatabaseParams {
//       database_id: database_id.to_string(),
//       views: vec![CreateViewParams {
//         database_id: database_id.to_string(),
//         view_id: "v1".to_string(),
//         ..Default::default()
//       }],
//       ..Default::default()
//     })
//     .await
//     .unwrap();
//
//   let duplicated_database = test.duplicate_database("v1", "v1_1").await.unwrap();
//   let mut db = duplicated_database.write().await;
//   let duplicated_view_id = db.get_first_database_view_id().unwrap();
//   let row_id = Uuid::new_v4();
//   db.create_row(CreateRowParams::new(row_id, database_id.to_string()))
//     .await
//     .unwrap();
//
//   assert_eq!(
//     db.get_rows_for_view(&duplicated_view_id, 20, None)
//       .await
//       .count()
//       .await,
//     1
//   );
//   assert_eq!(
//     database
//       .read()
//       .await
//       .get_rows_for_view("v1", 10, None)
//       .await
//       .count()
//       .await,
//     0
//   );
// }

#[tokio::test]
async fn duplicate_database_view_test() {
  // create the database with inline view
  let database_id = Uuid::new_v4();
  let mut database = create_database_with_params(CreateDatabaseParams {
    database_id: database_id.to_string(),
    views: vec![CreateViewParams {
      database_id: database_id.to_string(),
      view_id: "v1".to_string(),
      ..Default::default()
    }],
    ..Default::default()
  })
  .await;

  database
    .create_linked_view(CreateViewParams {
      database_id: database_id.to_string(),
      view_id: "v2".to_string(),
      ..Default::default()
    })
    .unwrap();

  // Duplicate the linked view.
  let duplicated_view = database.duplicate_linked_view("v2").unwrap();
  let row_id = Uuid::new_v4();
  database
    .create_row(CreateRowParams::new(row_id, database_id.to_string()))
    .await
    .unwrap();

  // Duplicated database should have the same rows as the original database
  assert_eq!(
    database.get_rows_for_view(&duplicated_view.id).await.len(),
    1
  );
  assert_eq!(database.get_rows_for_view("v1").await.len(), 1);
}

#[tokio::test]
async fn delete_database_linked_view_test() {
  let database_id = Uuid::new_v4();
  let mut database = create_database_with_params(CreateDatabaseParams {
    database_id: database_id.to_string(),
    views: vec![CreateViewParams {
      database_id: database_id.to_string(),
      view_id: "v1".to_string(),
      ..Default::default()
    }],
    ..Default::default()
  })
  .await;

  database
    .create_linked_view(CreateViewParams {
      database_id: database_id.to_string(),
      view_id: "v2".to_string(),
      ..Default::default()
    })
    .unwrap();

  let views = database.get_all_views();
  assert_eq!(views.len(), 2);

  database.delete_view("v2");

  let views = database.get_all_views();
  assert_eq!(views.len(), 1);

  database.delete_view("v1");

  let views = database.get_all_views();
  assert_eq!(views.len(), 0);
}

#[tokio::test]
async fn delete_database_inline_view_test() {
  let database_id = Uuid::new_v4();
  let mut database = create_database_with_params(CreateDatabaseParams {
    database_id: database_id.to_string(),
    views: vec![CreateViewParams {
      database_id: database_id.to_string(),
      view_id: "v1".to_string(),
      ..Default::default()
    }],
    ..Default::default()
  })
  .await;

  for i in 2..5 {
    database
      .create_linked_view(CreateViewParams {
        database_id: database_id.to_string(),
        view_id: format!("v{}", i),
        ..Default::default()
      })
      .unwrap();
  }

  // there should be 4 views: v1, v2, v3 and v4.
  let views = database.get_all_views();
  assert_eq!(views.len(), 4);

  database.delete_view("v1");
  let views = database.get_all_views();
  assert_eq!(views.len(), 3);
}

#[tokio::test]
async fn get_database_by_view_id_test() {
  let database_id = Uuid::new_v4();
  let mut database = create_database_with_params(CreateDatabaseParams {
    database_id: database_id.to_string(),
    views: vec![CreateViewParams {
      database_id: database_id.to_string(),
      view_id: "v1".to_string(),
      ..Default::default()
    }],
    ..Default::default()
  })
  .await;

  database
    .create_linked_view(CreateViewParams {
      database_id: database_id.to_string(),
      view_id: "v2".to_string(),
      ..Default::default()
    })
    .unwrap();

  let database = database.get_view("v2");
  assert!(database.is_some());
}
