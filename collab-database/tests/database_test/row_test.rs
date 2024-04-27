use collab_database::database::gen_row_id;
use collab_database::rows::{meta_id_from_row_id, CreateRowParams, DatabaseRow, RowId, RowMetaKey};
use collab_database::views::{CreateViewParams, OrderObjectPosition};
use uuid::Uuid;

use crate::database_test::helper::{
  create_database, create_database_with_default_data, create_row,
};

#[tokio::test]
async fn create_row_shared_by_two_view_test() {
  let database_id = uuid::Uuid::new_v4().to_string();
  let database_test = create_database(1, &database_id).await;
  let params = CreateViewParams {
    database_id: "1".to_string(),
    view_id: "v2".to_string(),
    ..Default::default()
  };
  database_test.create_linked_view(params).unwrap();

  let row_id = gen_row_id();
  database_test
    .create_row(CreateRowParams::new(row_id.clone(), database_id.clone()))
    .unwrap();

  let view_1 = database_test.views.get_view("v1").unwrap();
  let view_2 = database_test.views.get_view("v2").unwrap();
  assert_eq!(view_1.row_orders[0].id, row_id);
  assert_eq!(view_2.row_orders[0].id, row_id);
}

#[tokio::test]
async fn delete_row_shared_by_two_view_test() {
  let database_id = uuid::Uuid::new_v4().to_string();
  let database_test = create_database(1, &database_id).await;
  let params = CreateViewParams {
    database_id: "1".to_string(),
    view_id: "v2".to_string(),
    ..Default::default()
  };
  database_test.create_linked_view(params).unwrap();

  let row_order = database_test
    .create_row(CreateRowParams::new(gen_row_id(), database_id.clone()))
    .unwrap();
  database_test.remove_row(&row_order.id);

  let view_1 = database_test.views.get_view("v1").unwrap();
  let view_2 = database_test.views.get_view("v2").unwrap();
  assert!(view_1.row_orders.is_empty());
  assert!(view_2.row_orders.is_empty());
}

#[tokio::test]
async fn move_row_in_view_test() {
  let database_test = create_database_with_default_data(1, "1").await;
  let rows = database_test.get_rows_for_view("v1");
  assert_eq!(rows[0].id, 1.into());
  assert_eq!(rows[1].id, 2.into());
  assert_eq!(rows[2].id, 3.into());

  database_test.views.update_database_view("v1", |update| {
    update.move_row_order("3", "2");
  });

  let rows2 = database_test.get_rows_for_view("v1");
  assert_eq!(rows2[0].id, 1.into());
  assert_eq!(rows2[1].id, 3.into());
  assert_eq!(rows2[2].id, 2.into());

  database_test.views.update_database_view("v1", |update| {
    update.move_row_order("2", "1");
  });

  let row3 = database_test.get_rows_for_view("v1");
  assert_eq!(row3[0].id, 2.into());
  assert_eq!(row3[1].id, 1.into());
  assert_eq!(row3[2].id, 3.into());
}

#[tokio::test]
async fn move_row_in_views_test() {
  let database_test = create_database_with_default_data(1, "1").await;
  let params = CreateViewParams {
    database_id: "1".to_string(),
    view_id: "v2".to_string(),
    ..Default::default()
  };
  database_test.create_linked_view(params).unwrap();

  database_test.views.update_database_view("v1", |update| {
    update.move_row_order("3", "2");
  });

  let rows_1 = database_test.get_rows_for_view("v1");
  assert_eq!(rows_1[0].id, 1.into());
  assert_eq!(rows_1[1].id, 3.into());
  assert_eq!(rows_1[2].id, 2.into());

  let rows_2 = database_test.get_rows_for_view("v2");
  assert_eq!(rows_2[0].id, 1.into());
  assert_eq!(rows_2[1].id, 2.into());
  assert_eq!(rows_2[2].id, 3.into());
}

#[tokio::test]
async fn insert_row_in_views_test() {
  let database_id = uuid::Uuid::new_v4().to_string();
  let database_test = create_database_with_default_data(1, &database_id).await;
  let row = CreateRowParams::new(4, database_id.clone())
    .with_row_position(OrderObjectPosition::After(2.to_string()));
  database_test.create_row_in_view("v1", row);

  let rows = database_test.get_rows_for_view("v1");
  assert_eq!(rows[0].id, 1.into());
  assert_eq!(rows[1].id, 2.into());
  assert_eq!(rows[2].id, 4.into());
  assert_eq!(rows[3].id, 3.into());

  let row = CreateRowParams::new(5, database_id.clone())
    .with_row_position(OrderObjectPosition::Before(2.to_string()));
  database_test.create_row_in_view("v1", row);

  let rows = database_test.get_rows_for_view("v1");
  assert_eq!(rows[0].id, 1.into());
  assert_eq!(rows[1].id, 5.into());
  assert_eq!(rows[2].id, 2.into());
  assert_eq!(rows[3].id, 4.into());
  assert_eq!(rows[4].id, 3.into());

  let row = CreateRowParams::new(6, database_id.clone())
    .with_row_position(OrderObjectPosition::After(10.to_string()));
  database_test.create_row_in_view("v1", row);

  let rows = database_test.get_rows_for_view("v1");
  assert_eq!(rows[0].id, 1.into());
  assert_eq!(rows[1].id, 5.into());
  assert_eq!(rows[2].id, 2.into());
  assert_eq!(rows[3].id, 4.into());
  assert_eq!(rows[4].id, 3.into());
  assert_eq!(rows[5].id, 6.into());
}

#[tokio::test]
async fn insert_row_at_front_in_views_test() {
  let database_id = uuid::Uuid::new_v4().to_string();
  let database_test = create_database_with_default_data(1, &database_id).await;
  let row =
    CreateRowParams::new(4, database_id.clone()).with_row_position(OrderObjectPosition::Start);
  database_test.create_row_in_view("v1", row);

  let rows = database_test.get_rows_for_view("v1");
  assert_eq!(rows[0].id, 4.into());
  assert_eq!(rows[1].id, 1.into());
  assert_eq!(rows[2].id, 2.into());
  assert_eq!(rows[3].id, 3.into());
}

#[tokio::test]
async fn insert_row_at_last_in_views_test() {
  let database_id = uuid::Uuid::new_v4().to_string();
  let database_test = create_database_with_default_data(1, &database_id).await;
  let row = CreateRowParams::new(4, database_id.clone());
  database_test.create_row_in_view("v1", row);

  let rows = database_test.get_rows_for_view("v1");
  assert_eq!(rows[0].id, 1.into());
  assert_eq!(rows[1].id, 2.into());
  assert_eq!(rows[2].id, 3.into());
  assert_eq!(rows[3].id, 4.into());
}

#[tokio::test]
async fn duplicate_row_test() {
  let database_test = create_database_with_default_data(1, "1").await;
  let rows = database_test.get_rows_for_view("v1");
  assert_eq!(rows.len(), 3);

  let params = database_test.duplicate_row(&2.into()).unwrap();
  let (index, row_order) = database_test.create_row_in_view("v1", params).unwrap();
  assert_eq!(index, 2);

  let rows = database_test.get_rows_for_view("v1");
  assert_eq!(rows.len(), 4);

  assert_eq!(rows[0].id, 1.into());
  assert_eq!(rows[1].id, 2.into());
  assert_eq!(rows[2].id, row_order.id);
  assert_eq!(rows[3].id, 3.into());
}

#[tokio::test]
async fn duplicate_last_row_test() {
  let database_test = create_database_with_default_data(1, "1").await;
  let rows = database_test.get_rows_for_view("v1");
  assert_eq!(rows.len(), 3);

  let params = database_test.duplicate_row(&3.into()).unwrap();
  let (index, row_order) = database_test.create_row_in_view("v1", params).unwrap();
  assert_eq!(index, 3);

  let rows = database_test.get_rows_for_view("v1");
  assert_eq!(rows.len(), 4);
  assert_eq!(rows[3].id, row_order.id);
}

#[tokio::test]
async fn document_id_of_row_test() {
  let database_id = uuid::Uuid::new_v4().to_string();
  let database_test = create_database(1, &database_id).await;
  let row_id = Uuid::parse_str("43f6c30f-9d23-470c-a0dd-8819f08dcf2f").unwrap();
  let row_order = database_test
    .create_row(CreateRowParams::new(row_id, database_id.clone()))
    .unwrap();

  let row = database_test.get_row(&row_order.id);
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
  let database_test = create_database(1, &database_id).await;
  let row_id = Uuid::parse_str("43f6c30f-9d23-470c-a0dd-8819f08dcf2f").unwrap();
  let row_order = database_test
    .create_row(CreateRowParams::new(row_id, database_id.clone()))
    .unwrap();

  let row_meta_before = database_test.get_row_meta(&row_order.id).unwrap();
  assert!(row_meta_before.is_document_empty);

  database_test.update_row_meta(&row_order.id, |meta_update| {
    meta_update
      .insert_cover("cover 123")
      .insert_icon("icon 123")
      .update_is_document_empty(false);
  });

  let row_meta = database_test.get_row_meta(&row_order.id).unwrap();
  assert_eq!(row_meta.cover_url, Some("cover 123".to_string()));
  assert_eq!(row_meta.icon_url, Some("icon 123".to_string()));
  assert!(!row_meta.is_document_empty);
}

#[tokio::test]
async fn row_document_id_test() {
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
  let (collab, _) = create_row(1, RowId::from(1));
  assert!(DatabaseRow::validate(&collab.lock()).is_ok());
}
