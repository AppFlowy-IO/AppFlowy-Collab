use std::collections::HashMap;

use collab_database::fields::Field;
use collab_database::views::CreateViewParams;

use crate::database_test::helper::{
  create_database, create_database_with_default_data, TestFieldSetting,
};

#[tokio::test]
async fn new_field_new_field_setting_test() {
  let database_test = create_database_with_default_data(1, "1");
  let field_setting = TestFieldSetting::new();
  database_test.create_field(
    Field::new("f4".to_string(), "text field".to_string(), 0, true),
    field_setting.into(),
  );

  let field_settings: HashMap<String, TestFieldSetting> =
    database_test.get_field_settings("v1", None);
  assert_eq!(field_settings.len(), 4);
  assert!(TestFieldSetting::from(field_settings.get("f4").unwrap().to_owned()).is_visible);
}

// #[tokio::test]
// async fn remove_field_remove_field_setting_test() {
//   let database_test = create_database_with_default_data(1, "1");
//   database_test.create_field(Field::new(
//     "f4".to_string(),
//     "text field".to_string(),
//     0,
//     true,
//   ));

//   let field_settings: HashMap<String, TestFieldSetting> =
//     database_test.get_field_settings("v1", None);
//   assert_eq!(field_settings.len(), 2);
// }

// #[tokio::test]
// async fn update_field_setting_for_some_fields_test() {
//   let database_test = create_database_with_default_data(1, "1");
//   database_test.create_field(Field::new(
//     "f4".to_string(),
//     "text field".to_string(),
//     0,
//     true,
//   ));

//   let field_settings: HashMap<String, TestFieldSetting> =
//     database_test.get_field_settings("v1", None);
//   assert_eq!(field_settings.len(), 2);
// }

// #[tokio::test]
// async fn update_field_setting_for_all_fields_test() {
//   let database_test = create_database_with_default_data(1, "1");
//   database_test.create_field(Field::new(
//     "f4".to_string(),
//     "text field".to_string(),
//     0,
//     true,
//   ));

//   let field_settings: HashMap<String, TestFieldSetting> =
//     database_test.get_field_settings("v1", None);
//   assert_eq!(field_settings.len(), 2);
// }

// #[tokio::test]
// async fn new_view_new_field_setting_map_test() {
//   let database_test = create_database_with_default_data(1, "1");
//   database_test.create_field(Field::new(
//     "f4".to_string(),
//     "text field".to_string(),
//     0,
//     true,
//   ));

//   let field_settings: HashMap<String, TestFieldSetting> =
//     database_test.get_field_settings("v1", None);
//   assert_eq!(field_settings.len(), 2);
// }

// #[tokio::test]
// async fn new_view_new_field_setting_map_test() {
//   let database_test = create_database_with_default_data(1, "1");
//   database_test.create_field(Field::new(
//     "f4".to_string(),
//     "text field".to_string(),
//     0,
//     true,
//   ));

//   let field_settings: HashMap<String, TestFieldSetting> =
//     database_test.get_field_settings("v1", None);
//   assert_eq!(field_settings.len(), 2);
// }

// // duplicate a view, the field settings should be copied over

// // create a new view with a non-default field settings
