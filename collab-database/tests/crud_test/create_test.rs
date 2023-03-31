use crate::helper::{
  create_database, create_database_with_default_data, create_database_with_grid_view,
  make_grid_view,
};
use assert_json_diff::{assert_json_eq, assert_json_matches};
use collab_database::fields::Field;
use collab_database::rows::Row;
use collab_database::views::{CreateViewParams, Layout};
use nanoid::nanoid;
use serde_json::json;

#[test]
fn create_initial_database_test() {
  let database_test = create_database(1, "1");
  assert_json_eq!(
    json!({
      "fields": [],
      "rows": [],
      "views": []
    }),
    database_test.to_json_value()
  );
}

#[test]
fn create_database_with_single_view_test() {
  let database_test = create_database_with_default_data(1, "1");
  let params = CreateViewParams {
    id: "v1".to_string(),
    database_id: database_test.get_database_id().unwrap(),
    name: "my first grid".to_string(),
    layout: Layout::Grid,
    ..Default::default()
  };

  database_test.create_view(params);
  let view = database_test.views.get_view("v1").unwrap();
  assert_eq!(view.row_orders.len(), 3);
  assert_eq!(view.field_orders.len(), 3);
}

#[test]
fn create_same_database_view_twice_test() {
  let database_test = create_database_with_default_data(1, "1");
  let params = CreateViewParams {
    id: "v1".to_string(),
    database_id: database_test.get_database_id().unwrap(),
    name: "my first grid".to_string(),
    layout: Layout::Grid,
    ..Default::default()
  };
  database_test.create_view(params);

  let params = CreateViewParams {
    id: "v1".to_string(),
    database_id: database_test.get_database_id().unwrap(),
    name: "my second grid".to_string(),
    layout: Layout::Grid,
    ..Default::default()
  };
  database_test.create_view(params);
  let view = database_test.views.get_view("v1").unwrap();

  assert_eq!(view.name, "my second grid");
}

#[test]
fn create_database_row_test() {
  let database_test = create_database_with_grid_view(1, "1", "v1");

  let row_id = nanoid!(4);
  database_test.insert_row(Row {
    id: row_id.clone(),
    ..Default::default()
  });

  let view = database_test.views.get_view("v1").unwrap();
  assert_json_eq!(view.row_orders.last().unwrap().id, row_id);
}

#[test]
fn create_database_field_test() {
  let database_test = create_database_with_grid_view(1, "1", "v1");

  let field_id = nanoid!(4);
  database_test.insert_field(Field {
    id: field_id.clone(),
    name: "my third field".to_string(),
    ..Default::default()
  });

  let view = database_test.views.get_view("v1").unwrap();
  assert_json_eq!(view.field_orders.last().unwrap().id, field_id);
}
