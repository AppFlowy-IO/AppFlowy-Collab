use crate::database_test::helper::{
  DatabaseTest, create_database, default_field_settings_by_layout,
};
use crate::helper::{TestCheckboxTypeOption, TestDateFormat, TestDateTypeOption, TestTimeFormat};
use collab::util::AnyMapExt;
use collab_database::fields::{Field, TypeOptionDataBuilder, TypeOptions};
use collab_database::views::OrderObjectPosition;
use std::ops::DerefMut;
use uuid::Uuid;

#[tokio::test]
async fn insert_checkbox_type_option_data_test() {
  let mut test = user_database_with_default_field();
  test.update_field("f1", |field_update| {
    field_update.update_type_options(|type_option_update| {
      type_option_update.insert("0", TestCheckboxTypeOption { is_selected: true });
    });
  });

  let field = test.get_field("f1").unwrap();
  let type_option = field
    .get_type_option::<TestCheckboxTypeOption>("0")
    .unwrap();
  assert!(type_option.is_selected);
}

#[tokio::test]
async fn insert_date_type_option_data_test() {
  let mut test = user_database_with_default_field();
  let type_option = TestDateTypeOption {
    date_format: TestDateFormat::ISO,
    time_format: TestTimeFormat::TwelveHour,
    include_time: true,
  };
  test.update_field("f1", |field_update| {
    field_update.update_type_options(|type_option_update| {
      type_option_update.insert("0", type_option);
    });
  });

  let field = test.get_field("f1").unwrap();
  let type_option = field.get_type_option::<TestDateTypeOption>("0").unwrap();
  assert!(type_option.include_time);
  assert_eq!(type_option.date_format, TestDateFormat::ISO);
  assert_eq!(type_option.time_format, TestTimeFormat::TwelveHour);
}

#[tokio::test]
async fn update_date_type_option_data_test() {
  let mut test = user_database_with_default_field();
  let type_option = TestDateTypeOption {
    date_format: Default::default(),
    time_format: TestTimeFormat::TwelveHour,
    include_time: false,
  };
  test.update_field("f1", |field_update| {
    field_update.update_type_options(|type_option_update| {
      type_option_update.insert("0", type_option);
    });
  });

  test.update_field("f1", |field_update| {
    field_update.update_type_options(|type_option_update| {
      type_option_update.update(
        "0",
        TypeOptionDataBuilder::from([
          ("include_time".into(), true.into()),
          (
            "time_format".into(),
            TestTimeFormat::TwentyFourHour.value().into(),
          ),
        ]),
      );
    });
  });

  let field = test.get_field("f1").unwrap();
  let type_option = field.get_type_option::<TestDateTypeOption>("0").unwrap();
  assert!(type_option.include_time);
  assert_eq!(type_option.time_format, TestTimeFormat::TwentyFourHour);
}

#[tokio::test]
async fn single_field_contains_multiple_type_options_test() {
  let mut test = user_database_with_default_field();
  let date_tp = TestDateTypeOption {
    date_format: Default::default(),
    time_format: TestTimeFormat::TwelveHour,
    include_time: false,
  };

  let checkbox_tp = TestCheckboxTypeOption { is_selected: true };
  test.update_field("f1", |field_update| {
    field_update
      .set_field_type(0)
      .set_type_option(0, Some(checkbox_tp.into()));
  });

  test.update_field("f1", |field_update| {
    field_update
      .set_field_type(1)
      .set_type_option(1, Some(date_tp.into()));
  });

  let field = test.get_field("f1").unwrap();
  let check_tp = field
    .get_type_option::<TestCheckboxTypeOption>("0")
    .unwrap();
  let date_tp = field.get_type_option::<TestDateTypeOption>("1").unwrap();
  assert!(check_tp.is_selected);
  assert_eq!(date_tp.time_format, TestTimeFormat::TwelveHour);
}

#[tokio::test]
async fn insert_multi_type_options_test() {
  let mut test = user_database_with_default_field();

  let mut type_options = TypeOptions::new();
  type_options.insert(
    "0".to_string(),
    TypeOptionDataBuilder::from([("job 1".into(), 123.into())]),
  );
  type_options.insert(
    "1".to_string(),
    TypeOptionDataBuilder::from([("job 2".into(), (456.0).into())]),
  );

  test.create_field(
    None,
    Field {
      id: "f2".to_string(),
      name: "second field".to_string(),
      field_type: 0,
      type_options,
      ..Default::default()
    },
    &OrderObjectPosition::default(),
    default_field_settings_by_layout(),
  );

  let second_field = test.get_field("f2").unwrap();
  assert_eq!(second_field.type_options.len(), 2);

  let type_option = second_field.type_options.get("0").unwrap();
  assert_eq!(type_option.get_as::<i64>("job 1").unwrap(), 123);

  let type_option = second_field.type_options.get("1").unwrap();
  assert_eq!(type_option.get_as::<f64>("job 2").unwrap(), 456.0);
}

fn user_database_with_default_field() -> DatabaseTest {
  let database_id = Uuid::new_v4().to_string();
  let mut test = create_database(1, &database_id);

  let field = Field {
    id: "f1".to_string(),
    name: "first field".to_string(),
    field_type: 0,
    ..Default::default()
  };
  {
    let db = test.deref_mut();
    let mut txn = db.collab.transact_mut();
    db.body.fields.insert_field(&mut txn, field);
  }
  test
}
