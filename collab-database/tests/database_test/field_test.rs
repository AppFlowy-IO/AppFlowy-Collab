use crate::helper::create_database;
use collab_database::fields::Field;
use collab_database::views::CreateViewParams;

#[test]
fn create_single_field_test() {
  let database_test = create_database(1, "1");
  let params = CreateViewParams {
    view_id: "v1".to_string(),
    ..Default::default()
  };
  database_test.create_view(params);

  database_test.insert_field(Field::new(
    "f1".to_string(),
    "text field".to_string(),
    0,
    true,
  ));

  let fields = database_test.fields.get_all_fields();
  assert_eq!(fields.len(), 1);

  let view = database_test.views.get_view("v1").unwrap();
  assert_eq!(view.field_orders[0].id, fields[0].id);
}

#[test]
fn create_multiple_field_test() {
  let database_test = create_database(1, "1");
  for i in 0..10 {
    database_test.insert_field(Field::new(
      format!("f{}", i),
      format!("text field {}", i),
      0,
      true,
    ));
  }

  let fields = database_test.fields.get_all_fields();
  assert_eq!(fields.len(), 10);
}

#[test]
fn delete_field_test() {
  let database_test = create_database(1, "1");
  for i in 0..3 {
    database_test.insert_field(Field::new(
      format!("f{}", i),
      format!("text field {}", i),
      0,
      true,
    ));
  }
  database_test.delete_field("f0");
  database_test.delete_field("f1");
  let fields = database_test.fields.get_all_fields();
  assert_eq!(fields.len(), 1);
}

#[test]
fn delete_field_in_views_test() {
  let database_test = create_database(1, "1");
  for i in 0..3 {
    database_test.insert_field(Field::new(
      format!("f{}", i),
      format!("text field {}", i),
      0,
      true,
    ));
  }

  let params = CreateViewParams {
    view_id: "v1".to_string(),
    ..Default::default()
  };
  database_test.create_view(params);
  database_test.delete_field("f0");

  let fields = database_test.fields.get_all_fields();
  assert_eq!(fields.len(), 2);
  let view = database_test.views.get_view("v1").unwrap();
  assert_eq!(view.field_orders.len(), 2);
}

#[test]
fn field_order_in_view_test() {
  let database_test = create_database(1, "1");
  let params = CreateViewParams {
    view_id: "v1".to_string(),
    ..Default::default()
  };
  database_test.create_view(params);
  for i in 0..10 {
    database_test.insert_field(Field::new(
      format!("f{}", i),
      format!("text field {}", i),
      0,
      true,
    ));
  }

  let fields = database_test.fields.get_all_fields();
  assert_eq!(fields.len(), 10);

  let view = database_test.views.get_view("v1").unwrap();
  for i in 0..10 {
    assert_eq!(view.field_orders[i].id, format!("f{}", i));
  }
}

#[test]
fn move_field_test() {
  let database_test = create_database(1, "1");
  let params = CreateViewParams {
    view_id: "v1".to_string(),
    ..Default::default()
  };
  database_test.create_view(params);

  let params = CreateViewParams {
    view_id: "v2".to_string(),
    ..Default::default()
  };
  database_test.create_view(params);

  for i in 0..3 {
    database_test.insert_field(Field::new(
      format!("f{}", i),
      format!("text field {}", i),
      0,
      true,
    ));
  }

  database_test.views.update_view("v1", |update| {
    update.move_field_order(2, 0);
  });

  let view_1 = database_test.views.get_view("v1").unwrap();
  assert_eq!(view_1.field_orders[0].id, "f2");
  assert_eq!(view_1.field_orders[1].id, "f0");
  assert_eq!(view_1.field_orders[2].id, "f1");

  let view_2 = database_test.views.get_view("v2").unwrap();
  assert_eq!(view_2.field_orders[0].id, "f0");
  assert_eq!(view_2.field_orders[1].id, "f1");
  assert_eq!(view_2.field_orders[2].id, "f2");
}
