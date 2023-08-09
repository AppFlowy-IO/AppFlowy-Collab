use collab_database::fields::Field;
use collab_database::views::CreateViewParams;

use crate::database_test::helper::{
  create_database, create_database_with_default_data, TestFieldSetting,
};

#[tokio::test]
async fn create_single_field_test() {
  let database_test = create_database(1, "1");
  database_test.create_field(
    Field::new("f1".to_string(), "text field".to_string(), 0, true),
    TestFieldSetting::new().into(),
  );

  let fields = database_test.fields.get_all_fields();
  assert_eq!(fields.len(), 1);

  let view = database_test.views.get_view("v1").unwrap();
  assert_eq!(view.field_orders[0].id, fields[0].id);
}

#[tokio::test]
async fn duplicate_field_test() {
  let database_test = create_database_with_default_data(1, "1");
  let original_field = database_test.fields.get_field("f1").unwrap();
  let (index, duplicated_field) = database_test
    .duplicate_field("v1", "f1", |field| format!("{} (copy)", field.name))
    .unwrap();

  assert_eq!(index, 1);
  assert_ne!(original_field.id, duplicated_field.id);
  assert_eq!(
    duplicated_field.name,
    format!("{} (copy)", original_field.name)
  );
}

#[tokio::test]
async fn duplicate_field_test2() {
  let database_test = create_database_with_default_data(1, "1");
  let original_field = database_test.fields.get_field("f3").unwrap();
  let (index, duplicated_field) = database_test
    .duplicate_field("v1", "f3", |field| format!("{} (copy)", field.name))
    .unwrap();

  assert_eq!(index, 3);
  assert_ne!(original_field.id, duplicated_field.id);
  assert_eq!(
    duplicated_field.name,
    format!("{} (copy)", original_field.name)
  );
}

#[tokio::test]
async fn create_multiple_field_test() {
  let database_test = create_database(1, "1");
  for i in 0..10 {
    database_test.create_field(
      Field::new(format!("f{}", i), format!("text field {}", i), 0, true),
      TestFieldSetting::new().into(),
    );
  }

  let fields = database_test.fields.get_all_fields();
  assert_eq!(fields.len(), 10);
}

#[tokio::test]
async fn delete_field_test() {
  let database_test = create_database(1, "1");
  for i in 0..3 {
    database_test.create_field(
      Field::new(format!("f{}", i), format!("text field {}", i), 0, true),
      TestFieldSetting::new().into(),
    );
  }
  database_test.delete_field("f0");
  database_test.delete_field("f1");
  let fields = database_test.fields.get_all_fields();
  assert_eq!(fields.len(), 1);
}

#[tokio::test]
async fn delete_field_in_views_test() {
  let database_test = create_database(1, "1");
  for i in 0..3 {
    database_test.create_field(
      Field::new(format!("f{}", i), format!("text field {}", i), 0, true),
      TestFieldSetting::new().into(),
    );
  }

  let params = CreateViewParams {
    database_id: "1".to_string(),
    view_id: "v1".to_string(),
    ..Default::default()
  };
  database_test.create_linked_view(params).unwrap();
  database_test.delete_field("f0");

  let fields = database_test.fields.get_all_fields();
  assert_eq!(fields.len(), 2);
  let view = database_test.views.get_view("v1").unwrap();
  assert_eq!(view.field_orders.len(), 2);
}

#[tokio::test]
async fn field_order_in_view_test() {
  let database_test = create_database(1, "1");
  let params = CreateViewParams {
    database_id: "1".to_string(),
    view_id: "v1".to_string(),
    ..Default::default()
  };
  database_test.create_linked_view(params).unwrap();
  for i in 0..10 {
    database_test.create_field(
      Field::new(format!("f{}", i), format!("text field {}", i), 0, true),
      TestFieldSetting::new().into(),
    );
  }

  let fields = database_test.fields.get_all_fields();
  assert_eq!(fields.len(), 10);

  let view = database_test.views.get_view("v1").unwrap();
  for i in 0..10 {
    assert_eq!(view.field_orders[i].id, format!("f{}", i));
  }
}

#[tokio::test]
async fn get_field_in_order_test() {
  let database_test = create_database(1, "1");
  for i in 0..3 {
    database_test.create_field(
      Field::new(format!("f{}", i), format!("text field {}", i), 0, true),
      TestFieldSetting::new().into(),
    );
  }
  let fields = database_test.get_fields_in_view("v1", None);
  assert_eq!(fields[0].id, "f0");
  assert_eq!(fields[1].id, "f1");
  assert_eq!(fields[2].id, "f2");

  database_test.views.update_database_view("v1", |update| {
    update.move_field_order(0, 2);
  });
  let fields = database_test.get_fields_in_view("v1", None);
  assert_eq!(fields[0].id, "f1");
  assert_eq!(fields[1].id, "f2");
  assert_eq!(fields[2].id, "f0");
}

#[tokio::test]
async fn move_field_test() {
  let database_test = create_database(1, "1");
  let params = CreateViewParams {
    database_id: "1".to_string(),
    view_id: "v2".to_string(),
    ..Default::default()
  };
  database_test.create_linked_view(params).unwrap();

  for i in 0..3 {
    database_test.create_field(
      Field::new(format!("f{}", i), format!("text field {}", i), 0, true),
      TestFieldSetting::new().into(),
    );
  }

  database_test.views.update_database_view("v1", |update| {
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

#[tokio::test]
async fn move_field_to_out_of_index_test() {
  let database_test = create_database(1, "1");
  for i in 0..3 {
    database_test.create_field(
      Field::new(format!("f{}", i), format!("text field {}", i), 0, true),
      TestFieldSetting::new().into(),
    );
  }

  database_test.views.update_database_view("v1", |update| {
    update.move_field_order(2, 10);
  });
  let view_1 = database_test.views.get_view("v1").unwrap();
  assert_eq!(view_1.field_orders[0].id, "f0");
  assert_eq!(view_1.field_orders[1].id, "f1");
  assert_eq!(view_1.field_orders[2].id, "f2");

  database_test.views.update_database_view("v1", |update| {
    update.move_field_order(10, 1);
  });
  let view_1 = database_test.views.get_view("v1").unwrap();
  assert_eq!(view_1.field_orders[0].id, "f0");
  assert_eq!(view_1.field_orders[1].id, "f1");
  assert_eq!(view_1.field_orders[2].id, "f2");
}
