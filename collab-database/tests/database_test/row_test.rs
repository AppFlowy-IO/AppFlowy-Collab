use crate::database_test::helper::{
  create_database, create_database_with_default_data, create_row,
};
use collab::core::collab::default_client_id;
use collab_database::database::gen_row_id;
use collab_database::entity::{CreateViewParams, FileUploadType};
use collab_database::rows::{
  CoverType, CreateRowParams, RowCover, RowId, RowMetaKey, meta_id_from_row_id,
};
use collab_database::views::OrderObjectPosition;
use uuid::Uuid;

#[tokio::test]
async fn create_row_shared_by_two_view_test() {
  let database_id = uuid::Uuid::new_v4().to_string();
  let mut database_test = create_database(1, &database_id);
  let params = CreateViewParams {
    database_id: "1".to_string(),
    view_id: "v2".to_string(),
    ..Default::default()
  };
  database_test.create_linked_view(params).unwrap();

  let row_id = gen_row_id();
  database_test
    .create_row(CreateRowParams::new(row_id.clone(), database_id.clone()))
    .await
    .unwrap();

  let view_1 = database_test.get_view("v1").unwrap();
  let view_2 = database_test.get_view("v2").unwrap();

  for row_order in view_1.row_orders.iter() {
    let _ = database_test.get_row_detail(&row_order.id).await.unwrap();
  }

  for row_order in view_2.row_orders.iter() {
    let _ = database_test.get_row_detail(&row_order.id).await.unwrap();
  }

  assert_eq!(view_1.row_orders[0].id, row_id);
  assert_eq!(view_2.row_orders[0].id, row_id);
}

#[tokio::test]
async fn delete_row_shared_by_two_view_test() {
  let database_id = uuid::Uuid::new_v4().to_string();
  let mut database_test = create_database(1, &database_id);
  let params = CreateViewParams {
    database_id: "1".to_string(),
    view_id: "v2".to_string(),
    ..Default::default()
  };
  database_test.create_linked_view(params).unwrap();

  let row_order = database_test
    .create_row(CreateRowParams::new(gen_row_id(), database_id.clone()))
    .await
    .unwrap();
  database_test.remove_row(&row_order.id).await;

  let view_1 = database_test.get_view("v1").unwrap();
  let view_2 = database_test.get_view("v2").unwrap();
  assert!(view_1.row_orders.is_empty());
  assert!(view_2.row_orders.is_empty());
}

#[tokio::test]
async fn move_row_in_view_test() {
  let database_id = uuid::Uuid::new_v4();
  let mut database_test = create_database_with_default_data(1, &database_id.to_string()).await;
  let rows = database_test.get_rows_for_view("v1").await;
  let first_row_id = database_test.pre_define_row_ids[0].clone();
  let second_row_id = database_test.pre_define_row_ids[1].clone();
  let third_row_id = database_test.pre_define_row_ids[2].clone();

  assert_eq!(rows[0].id, first_row_id);
  assert_eq!(rows[1].id, second_row_id);
  assert_eq!(rows[2].id, third_row_id);

  database_test.update_database_view("v1", |update| {
    update.move_row_order(third_row_id.as_str(), second_row_id.as_str());
  });

  let rows2 = database_test.get_rows_for_view("v1").await;
  assert_eq!(rows2[0].id, first_row_id);
  assert_eq!(rows2[1].id, third_row_id);
  assert_eq!(rows2[2].id, second_row_id);

  database_test.update_database_view("v1", |update| {
    update.move_row_order(second_row_id.as_str(), first_row_id.as_str());
  });

  let row3 = database_test.get_rows_for_view("v1").await;
  assert_eq!(row3[0].id, second_row_id);
  assert_eq!(row3[1].id, first_row_id);
  assert_eq!(row3[2].id, third_row_id);
}

#[tokio::test]
async fn move_row_in_view_test2() {
  let database_id = uuid::Uuid::new_v4();
  let mut database_test = create_database_with_default_data(1, &database_id.to_string()).await;
  let rows = database_test.get_rows_for_view("v1").await;
  let first_row_id = database_test.pre_define_row_ids[0].clone();
  let second_row_id = database_test.pre_define_row_ids[1].clone();
  let third_row_id = database_test.pre_define_row_ids[2].clone();

  assert_eq!(rows[0].id, first_row_id);
  assert_eq!(rows[1].id, second_row_id);
  assert_eq!(rows[2].id, third_row_id);

  database_test.update_database_view("v1", |update| {
    update.move_row_order(first_row_id.as_str(), third_row_id.as_str());
  });

  let rows2 = database_test.get_rows_for_view("v1").await;
  assert_eq!(rows2[0].id, second_row_id);
  assert_eq!(rows2[1].id, third_row_id);
  assert_eq!(rows2[2].id, first_row_id);
}

#[tokio::test]
async fn move_row_in_views_test() {
  let database_id = uuid::Uuid::new_v4();
  let mut database_test = create_database_with_default_data(1, &database_id.to_string()).await;
  let params = CreateViewParams {
    database_id: database_id.to_string(),
    view_id: "v2".to_string(),
    ..Default::default()
  };
  database_test.create_linked_view(params).unwrap();

  let first_row_id = database_test.pre_define_row_ids[0].clone();
  let second_row_id = database_test.pre_define_row_ids[1].clone();
  let third_row_id = database_test.pre_define_row_ids[2].clone();

  database_test.update_database_view("v1", |update| {
    update.move_row_order(third_row_id.as_str(), second_row_id.as_str());
  });

  let rows_1 = database_test.get_rows_for_view("v1").await;
  assert_eq!(rows_1[0].id, first_row_id);
  assert_eq!(rows_1[1].id, third_row_id);
  assert_eq!(rows_1[2].id, second_row_id);

  let rows_2 = database_test.get_rows_for_view("v2").await;
  assert_eq!(rows_2[0].id, first_row_id);
  assert_eq!(rows_2[1].id, second_row_id);
  assert_eq!(rows_2[2].id, third_row_id);
}

#[tokio::test]
async fn insert_row_in_views_test() {
  let database_id = uuid::Uuid::new_v4().to_string();
  let mut database_test = create_database_with_default_data(1, &database_id).await;
  let first_row_id = database_test.pre_define_row_ids[0].clone();
  let second_row_id = database_test.pre_define_row_ids[1].clone();
  let third_row_id = database_test.pre_define_row_ids[2].clone();
  let fourth_row_id = gen_row_id();
  let row = CreateRowParams::new(fourth_row_id.clone(), database_id.clone())
    .with_row_position(OrderObjectPosition::After(second_row_id.to_string()));
  database_test.create_row_in_view("v1", row).await.unwrap();

  let rows = database_test.get_rows_for_view("v1").await;
  assert_eq!(rows[0].id, first_row_id);
  assert_eq!(rows[1].id, second_row_id);
  assert_eq!(rows[2].id, fourth_row_id);
  assert_eq!(rows[3].id, third_row_id);

  let fifth_row_id = gen_row_id();
  let row = CreateRowParams::new(fifth_row_id.clone(), database_id.clone())
    .with_row_position(OrderObjectPosition::Before(second_row_id.to_string()));
  database_test.create_row_in_view("v1", row).await.unwrap();

  let rows = database_test.get_rows_for_view("v1").await;
  assert_eq!(rows[0].id, first_row_id);
  assert_eq!(rows[1].id, fifth_row_id);
  assert_eq!(rows[2].id, second_row_id);
  assert_eq!(rows[3].id, fourth_row_id);
  assert_eq!(rows[4].id, third_row_id);

  let sixth_row_id = gen_row_id();
  let row = CreateRowParams::new(sixth_row_id.clone(), database_id.clone())
    .with_row_position(OrderObjectPosition::After(10.to_string()));
  database_test.create_row_in_view("v1", row).await.unwrap();

  let rows = database_test.get_rows_for_view("v1").await;

  assert_eq!(rows[0].id, first_row_id);
  assert_eq!(rows[1].id, fifth_row_id);
  assert_eq!(rows[2].id, second_row_id);
  assert_eq!(rows[3].id, fourth_row_id);
  assert_eq!(rows[4].id, third_row_id);
  assert_eq!(rows[5].id, sixth_row_id);
}

#[tokio::test]
async fn insert_row_at_front_in_views_test() {
  let database_id = uuid::Uuid::new_v4().to_string();
  let mut database_test = create_database_with_default_data(1, &database_id).await;

  let new_row_id = gen_row_id();
  let row = CreateRowParams::new(new_row_id.clone(), database_id.clone())
    .with_row_position(OrderObjectPosition::Start);
  database_test.create_row_in_view("v1", row).await.unwrap();

  let rows = database_test.get_rows_for_view("v1").await;
  assert_eq!(rows[0].id, new_row_id);
  assert_eq!(rows[1].id, database_test.pre_define_row_ids[0]);
  assert_eq!(rows[2].id, database_test.pre_define_row_ids[1]);
  assert_eq!(rows[3].id, database_test.pre_define_row_ids[2]);
}

#[tokio::test]
async fn insert_row_at_last_in_views_test() {
  let database_id = uuid::Uuid::new_v4().to_string();
  let mut database_test = create_database_with_default_data(1, &database_id).await;

  let fourth_row_id = gen_row_id();
  let row = CreateRowParams::new(fourth_row_id.clone(), database_id.clone());
  database_test.create_row_in_view("v1", row).await.unwrap();

  let rows = database_test.get_rows_for_view("v1").await;
  let first_row_id = database_test.pre_define_row_ids[0].clone();
  let second_row_id = database_test.pre_define_row_ids[1].clone();
  let third_row_id = database_test.pre_define_row_ids[2].clone();
  assert_eq!(rows[0].id, first_row_id);
  assert_eq!(rows[1].id, second_row_id);
  assert_eq!(rows[2].id, third_row_id);
  assert_eq!(rows[3].id, fourth_row_id);
}

#[tokio::test]
async fn duplicate_row_test() {
  let database_id = uuid::Uuid::new_v4();
  let mut database_test = create_database_with_default_data(1, &database_id.to_string()).await;
  let rows = database_test.get_rows_for_view("v1").await;
  assert_eq!(rows.len(), 3);
  let first_row_id = database_test.pre_define_row_ids[0].clone();
  let second_row_id = database_test.pre_define_row_ids[1].clone();
  let third_row_id = database_test.pre_define_row_ids[2].clone();

  let params = database_test.duplicate_row(&second_row_id).await.unwrap();
  let (index, row_order) = database_test
    .create_row_in_view("v1", params)
    .await
    .unwrap();
  assert_eq!(index, 2);

  let rows = database_test.get_rows_for_view("v1").await;
  assert_eq!(rows.len(), 4);

  assert_eq!(rows[0].id, first_row_id);
  assert_eq!(rows[1].id, second_row_id);
  assert_eq!(rows[2].id, row_order.id);
  assert_eq!(rows[3].id, third_row_id);
}

#[tokio::test]
async fn duplicate_last_row_test() {
  let database_id = uuid::Uuid::new_v4();
  let mut database_test = create_database_with_default_data(1, &database_id.to_string()).await;
  let rows = database_test.get_rows_for_view("v1").await;
  assert_eq!(rows.len(), 3);

  let params = database_test
    .duplicate_row(&database_test.pre_define_row_ids[2].clone())
    .await
    .unwrap();
  let (index, row_order) = database_test
    .create_row_in_view("v1", params)
    .await
    .unwrap();
  assert_eq!(index, 3);

  let rows = database_test.get_rows_for_view("v1").await;
  assert_eq!(rows.len(), 4);
  assert_eq!(rows[3].id, row_order.id);
}

#[tokio::test]
async fn document_id_of_row_test() {
  let database_id = uuid::Uuid::new_v4().to_string();
  let mut database_test = create_database(1, &database_id);
  let row_id = Uuid::parse_str("43f6c30f-9d23-470c-a0dd-8819f08dcf2f").unwrap();
  let row_order = database_test
    .create_row(CreateRowParams::new(row_id, database_id.clone()))
    .await
    .unwrap();

  let row = database_test.get_row(&row_order.id).await;
  let expected_document_id = meta_id_from_row_id(
    &Uuid::parse_str(row.id.as_str()).unwrap(),
    RowMetaKey::DocumentId,
  );
  assert_eq!(row.document_id(), expected_document_id,);
  assert_eq!(row.document_id(), expected_document_id,);
}

#[tokio::test]
async fn update_row_meta_test() {
  let database_id = uuid::Uuid::new_v4().to_string();
  let mut database_test = create_database(1, &database_id);
  let row_id = Uuid::parse_str("43f6c30f-9d23-470c-a0dd-8819f08dcf2f").unwrap();
  let row_order = database_test
    .create_row(CreateRowParams::new(row_id, database_id.clone()))
    .await
    .unwrap();

  let row_meta_before = database_test.get_row_meta(&row_order.id).await.unwrap();
  assert!(row_meta_before.is_document_empty);

  let cover = RowCover {
    data: "cover 123".to_string(),
    upload_type: FileUploadType::LocalFile,
    cover_type: CoverType::FileCover,
  };

  database_test
    .update_row_meta(&row_order.id, |meta_update| {
      meta_update
        .insert_cover(&cover)
        .insert_icon("icon 123")
        .update_is_document_empty(false);
    })
    .await;

  let row_meta = database_test.get_row_meta(&row_order.id).await.unwrap();
  let cover = row_meta.cover.unwrap();
  assert_eq!(cover.data, "cover 123".to_string());
  assert_eq!(row_meta.icon_url, Some("icon 123".to_string()));
  assert!(!row_meta.is_document_empty);
}

// #[tokio::test]
// async fn update_row_id_test() {
//   let database_id = uuid::Uuid::new_v4().to_string();
//   let mut database_test = create_database(1, &database_id);
//   let row_id = uuid::Uuid::new_v4().to_string();
//   let row_order = database_test
//     .create_row(CreateRowParams::new(row_id.clone(), database_id.clone()))
//     .await
//     .unwrap();
//
//   database_test
//     .update_row_meta(&row_order.id, |meta_update| {
//       meta_update
//         .insert_cover(&RowCover {
//           data: "cover1".to_string(),
//           upload_type: FileUploadType::LocalFile,
//           cover_type: CoverType::FileCover,
//         })
//         .insert_icon("icon1")
//         .update_is_document_empty(false)
//         .update_attachment_count(10);
//     })
//     .await;
//
//   let row_meta = database_test.get_row_meta(&row_order.id).await.unwrap();
//
//   // Update row
//   let new_row_id = uuid::Uuid::new_v4().to_string();
//   database_test
//     .update_row(row_order.id, |row_update| {
//       row_update.set_row_id(new_row_id.clone().into());
//     })
//     .await;
//
//   // Check if the new row has the same meta data as the old row
//   let new_row_meta = database_test
//     .get_row_meta(&new_row_id.clone().into())
//     .await
//     .unwrap();
//   assert_eq!(
//     new_row_meta.cover.clone().unwrap().data,
//     row_meta.cover.unwrap().data
//   );
//   assert_eq!(new_row_meta.icon_url.unwrap(), row_meta.icon_url.unwrap());
//   assert_eq!(new_row_meta.is_document_empty, row_meta.is_document_empty);
//   assert_eq!(new_row_meta.attachment_count, row_meta.attachment_count);
// }

#[test]
fn row_document_id_test() {
  for _ in 0..10 {
    let namespace = Uuid::parse_str("43f6c30f-9d23-470c-a0dd-8819f08dcf2f").unwrap();
    let derived_uuid = Uuid::new_v5(&namespace, b"document_id");
    assert_eq!(
      derived_uuid.to_string(),
      "0b1903ac-0dc2-5643-b0b5-a3f893cac26b".to_string()
    );
  }
}

#[tokio::test]
async fn validate_row_test() {
  let workspace_id = Uuid::new_v4().to_string();
  let row = create_row(1, &workspace_id, RowId::from(1), default_client_id());
  row.validate().unwrap();
}
