use collab_database::database::gen_row_id;
use collab_database::rows::CreateRowParams;
use collab_database::views::CreateViewParams;

use crate::database_test::helper::{create_database, create_database_with_default_data};

#[test]
fn create_row_shared_by_two_view_test() {
  let database_test = create_database(1, "1");
  let params = CreateViewParams {
    view_id: "v2".to_string(),
    ..Default::default()
  };
  database_test.create_linked_view(params);

  let row_id = gen_row_id();
  database_test.create_row(CreateRowParams {
    id: row_id.clone(),
    ..Default::default()
  });

  let view_1 = database_test.views.get_view("v1").unwrap();
  let view_2 = database_test.views.get_view("v2").unwrap();
  assert_eq!(view_1.row_orders[0].id, row_id);
  assert_eq!(view_2.row_orders[0].id, row_id);
}

#[test]
fn delete_row_shared_by_two_view_test() {
  let database_test = create_database(1, "1");
  let params = CreateViewParams {
    view_id: "v2".to_string(),
    ..Default::default()
  };
  database_test.create_linked_view(params);

  let row_order = database_test
    .create_row(CreateRowParams {
      id: gen_row_id(),
      ..Default::default()
    })
    .unwrap();
  database_test.remove_row(&row_order.id);

  let view_1 = database_test.views.get_view("v1").unwrap();
  let view_2 = database_test.views.get_view("v2").unwrap();
  assert!(view_1.row_orders.is_empty());
  assert!(view_2.row_orders.is_empty());
}

#[test]
fn move_row_in_view_test() {
  let database_test = create_database_with_default_data(1, "1");
  let rows = database_test.get_rows_for_view("v1");
  assert_eq!(rows[0].id, 1.into());
  assert_eq!(rows[1].id, 2.into());
  assert_eq!(rows[2].id, 3.into());

  database_test.views.update_database_view("v1", |update| {
    update.move_row_order(2, 1);
  });

  let rows2 = database_test.get_rows_for_view("v1");
  assert_eq!(rows2[0].id, 1.into());
  assert_eq!(rows2[1].id, 3.into());
  assert_eq!(rows2[2].id, 2.into());

  database_test.views.update_database_view("v1", |update| {
    update.move_row_order(2, 0);
  });

  let row3 = database_test.get_rows_for_view("v1");
  assert_eq!(row3[0].id, 2.into());
  assert_eq!(row3[1].id, 1.into());
  assert_eq!(row3[2].id, 3.into());
}

#[test]
fn move_row_in_views_test() {
  let database_test = create_database_with_default_data(1, "1");
  let params = CreateViewParams {
    view_id: "v2".to_string(),
    ..Default::default()
  };
  database_test.create_linked_view(params);

  database_test.views.update_database_view("v1", |update| {
    update.move_row_order(2, 1);
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

#[test]
fn insert_row_in_views_test() {
  let database_test = create_database_with_default_data(1, "1");
  let row = CreateRowParams {
    id: 4.into(),
    prev_row_id: Some(2.into()),
    ..Default::default()
  };
  database_test.create_row_in_view("v1", row);

  let rows = database_test.get_rows_for_view("v1");
  assert_eq!(rows[0].id, 1.into());
  assert_eq!(rows[1].id, 2.into());
  assert_eq!(rows[2].id, 4.into());
  assert_eq!(rows[3].id, 3.into());
}

#[test]
fn insert_row_at_front_in_views_test() {
  let database_test = create_database_with_default_data(1, "1");
  let row = CreateRowParams {
    id: 4.into(),
    ..Default::default()
  };
  database_test.create_row_in_view("v1", row);

  let rows = database_test.get_rows_for_view("v1");
  assert_eq!(rows[0].id, 4.into());
  assert_eq!(rows[1].id, 1.into());
  assert_eq!(rows[2].id, 2.into());
  assert_eq!(rows[3].id, 3.into());
}

#[test]
fn insert_row_at_last_in_views_test() {
  let database_test = create_database_with_default_data(1, "1");
  let row = CreateRowParams {
    id: 4.into(),
    prev_row_id: Some(3.into()),
    ..Default::default()
  };
  database_test.create_row_in_view("v1", row);

  let rows = database_test.get_rows_for_view("v1");
  assert_eq!(rows[0].id, 1.into());
  assert_eq!(rows[1].id, 2.into());
  assert_eq!(rows[2].id, 3.into());
  assert_eq!(rows[3].id, 4.into());
}

#[test]
fn duplicate_row_test() {
  let database_test = create_database_with_default_data(1, "1");
  let rows = database_test.get_rows_for_view("v1");
  assert_eq!(rows.len(), 3);

  let (index, row_order) = database_test.duplicate_row("v1", &2.into()).unwrap();
  assert_eq!(index, 2);

  let rows = database_test.get_rows_for_view("v1");
  assert_eq!(rows.len(), 4);

  assert_eq!(rows[0].id, 1.into());
  assert_eq!(rows[1].id, 2.into());
  assert_eq!(rows[2].id, row_order.id);
  assert_eq!(rows[3].id, 3.into());
}

#[test]
fn duplicate_last_row_test() {
  let database_test = create_database_with_default_data(1, "1");
  let rows = database_test.get_rows_for_view("v1");
  assert_eq!(rows.len(), 3);

  let (index, row_order) = database_test.duplicate_row("v1", &3.into()).unwrap();
  assert_eq!(index, 3);

  let rows = database_test.get_rows_for_view("v1");
  assert_eq!(rows.len(), 4);
  assert_eq!(rows[3].id, row_order.id);
}
