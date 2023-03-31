use crate::helper::{create_database, create_database_with_default_data};
use collab_database::rows::Row;
use collab_database::views::CreateViewParams;
use nanoid::nanoid;

#[test]
fn create_row_shared_by_two_view_test() {
  let database_test = create_database(1, "1");
  let params = CreateViewParams {
    id: "v1".to_string(),
    ..Default::default()
  };
  database_test.create_view(params);

  let params = CreateViewParams {
    id: "v2".to_string(),
    ..Default::default()
  };
  database_test.create_view(params);

  let row_id = nanoid!(4);
  database_test.insert_row(Row {
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
    id: "v1".to_string(),
    ..Default::default()
  };
  database_test.create_view(params);

  let params = CreateViewParams {
    id: "v2".to_string(),
    ..Default::default()
  };
  database_test.create_view(params);

  let row_id = nanoid!(4);
  database_test.insert_row(Row {
    id: row_id.clone(),
    ..Default::default()
  });

  database_test.delete_row(&row_id);

  let view_1 = database_test.views.get_view("v1").unwrap();
  let view_2 = database_test.views.get_view("v2").unwrap();
  let rows = database_test.rows.get_all_rows();
  assert!(view_1.row_orders.is_empty());
  assert!(view_2.row_orders.is_empty());
  assert!(rows.is_empty())
}

#[test]
fn move_row_in_view_test() {
  let database_test = create_database_with_default_data(1, "1");
  let params = CreateViewParams {
    id: "v1".to_string(),
    ..Default::default()
  };
  database_test.create_view(params);

  let rows = database_test.get_rows_for_view("v1");
  assert_eq!(rows[0].id, "r1");
  assert_eq!(rows[1].id, "r2");
  assert_eq!(rows[2].id, "r3");

  database_test.views.update_view("v1", |update| {
    update.move_row(2, 1);
  });

  let rows2 = database_test.get_rows_for_view("v1");
  assert_eq!(rows2[0].id, "r1");
  assert_eq!(rows2[1].id, "r3");
  assert_eq!(rows2[2].id, "r2");

  database_test.views.update_view("v1", |update| {
    update.move_row(2, 0);
  });

  let row3 = database_test.get_rows_for_view("v1");
  assert_eq!(row3[0].id, "r2");
  assert_eq!(row3[1].id, "r1");
  assert_eq!(row3[2].id, "r3");
}

#[test]
fn move_row_in_views_test() {
  let database_test = create_database_with_default_data(1, "1");
  let params = CreateViewParams {
    id: "v1".to_string(),
    ..Default::default()
  };
  database_test.create_view(params);
  let params = CreateViewParams {
    id: "v2".to_string(),
    ..Default::default()
  };
  database_test.create_view(params);

  database_test.views.update_view("v1", |update| {
    update.move_row(2, 1);
  });

  let rows_1 = database_test.get_rows_for_view("v1");
  assert_eq!(rows_1[0].id, "r1");
  assert_eq!(rows_1[1].id, "r3");
  assert_eq!(rows_1[2].id, "r2");

  let rows_2 = database_test.get_rows_for_view("v2");
  assert_eq!(rows_2[0].id, "r1");
  assert_eq!(rows_2[1].id, "r2");
  assert_eq!(rows_2[2].id, "r3");
}
