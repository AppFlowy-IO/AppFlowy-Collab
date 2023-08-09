use collab::core::any_map::AnyMapExtension;
use collab_database::fields::{Field, TypeOptionDataBuilder, TypeOptions};

use crate::database_test::helper::{create_database, DatabaseTest, TestFieldSetting};
use crate::helper::{TestCheckboxTypeOption, TestDateFormat, TestDateTypeOption, TestTimeFormat};

#[tokio::test]
async fn insert_checkbox_type_option_data_test() {
  let test = user_database_with_default_field();
  test.fields.update_field("f1", |field_update| {
    field_update.update_type_options(|type_option_update| {
      type_option_update.insert("0", TestCheckboxTypeOption { is_selected: true });
    });
  });

  let field = test.fields.get_field("f1").unwrap();
  let type_option = field
    .get_type_option::<TestCheckboxTypeOption>("0")
    .unwrap();
  assert!(type_option.is_selected);
}

#[tokio::test]
async fn insert_date_type_option_data_test() {
  let test = user_database_with_default_field();
  let type_option = TestDateTypeOption {
    date_format: TestDateFormat::ISO,
    time_format: TestTimeFormat::TwelveHour,
    include_time: true,
  };
  test.fields.update_field("f1", |field_update| {
    field_update.update_type_options(|type_option_update| {
      type_option_update.insert("0", type_option);
    });
  });

  let field = test.fields.get_field("f1").unwrap();
  let type_option = field.get_type_option::<TestDateTypeOption>("0").unwrap();
  assert!(type_option.include_time);
  assert_eq!(type_option.date_format, TestDateFormat::ISO);
  assert_eq!(type_option.time_format, TestTimeFormat::TwelveHour);
}

#[tokio::test]
async fn update_date_type_option_data_test() {
  let test = user_database_with_default_field();
  let type_option = TestDateTypeOption {
    date_format: Default::default(),
    time_format: TestTimeFormat::TwelveHour,
    include_time: false,
  };
  test.fields.update_field("f1", |field_update| {
    field_update.update_type_options(|type_option_update| {
      type_option_update.insert("0", type_option);
    });
  });

  test.fields.update_field("f1", |field_update| {
    field_update.update_type_options(|type_option_update| {
      type_option_update.update(
        "0",
        TypeOptionDataBuilder::new()
          .insert_bool_value("include_time", true)
          .insert_i64_value("time_format", TestTimeFormat::TwentyFourHour.value())
          .build(),
      );
    });
  });

  let field = test.fields.get_field("f1").unwrap();
  let type_option = field.get_type_option::<TestDateTypeOption>("0").unwrap();
  assert!(type_option.include_time);
  assert_eq!(type_option.time_format, TestTimeFormat::TwentyFourHour);
}

#[tokio::test]
async fn single_field_contains_multiple_type_options_test() {
  let test = user_database_with_default_field();
  let date_tp = TestDateTypeOption {
    date_format: Default::default(),
    time_format: TestTimeFormat::TwelveHour,
    include_time: false,
  };

  let checkbox_tp = TestCheckboxTypeOption { is_selected: true };
  test.fields.update_field("f1", |field_update| {
    field_update
      .set_field_type(0)
      .set_type_option(0, Some(checkbox_tp.into()));
  });

  test.fields.update_field("f1", |field_update| {
    field_update
      .set_field_type(1)
      .set_type_option(1, Some(date_tp.into()));
  });

  let field = test.fields.get_field("f1").unwrap();
  let check_tp = field
    .get_type_option::<TestCheckboxTypeOption>("0")
    .unwrap();
  let date_tp = field.get_type_option::<TestDateTypeOption>("1").unwrap();
  assert!(check_tp.is_selected);
  assert_eq!(date_tp.time_format, TestTimeFormat::TwelveHour);
}

#[tokio::test]
async fn insert_multi_type_options_test() {
  let test = user_database_with_default_field();

  let mut type_options = TypeOptions::new();
  type_options.insert(
    "0".to_string(),
    TypeOptionDataBuilder::new()
      .insert_i64_value("job 1", 123)
      .build(),
  );
  type_options.insert(
    "1".to_string(),
    TypeOptionDataBuilder::new()
      .insert_f64_value("job 2", 456.0)
      .build(),
  );

  test.create_field(
    Field {
      id: "f2".to_string(),
      name: "second field".to_string(),
      field_type: 0,
      type_options,
      ..Default::default()
    },
    TestFieldSetting::new().into(),
  );

  let second_field = test.fields.get_field("f2").unwrap();
  assert_eq!(second_field.type_options.len(), 2);

  let type_option = second_field.type_options.get("0").unwrap();
  assert_eq!(type_option.get_i64_value("job 1").unwrap(), 123);

  let type_option = second_field.type_options.get("1").unwrap();
  assert_eq!(type_option.get_f64_value("job 2").unwrap(), 456.0);
}

fn user_database_with_default_field() -> DatabaseTest {
  let test = create_database(1, "1");

  let field = Field {
    id: "f1".to_string(),
    name: "first field".to_string(),
    field_type: 0,
    ..Default::default()
  };
  test.fields.insert_field(field);
  test
}
