use crate::database_test::helper::{
  create_database_with_default_data, default_field_settings_by_layout,
  field_settings_for_default_database,
};
use crate::helper::TestFieldSetting;
use collab_database::entity::CreateViewParams;
use collab_database::fields::Field;
use collab_database::views::{DatabaseLayout, OrderObjectPosition};
use std::collections::HashMap;

#[tokio::test]
async fn new_field_new_field_setting_test() {
  let database_id = uuid::Uuid::new_v4();
  let mut database_test = create_database_with_default_data(1, &database_id.to_string()).await;
  let params = CreateViewParams {
    database_id: database_id.to_string(),
    view_id: "v2".to_string(),
    field_settings: field_settings_for_default_database(),
    ..Default::default()
  };

  database_test.create_linked_view(params).unwrap();

  // Create a new field
  database_test.create_field(
    None,
    Field::new("f4".to_string(), "text field".to_string(), 0, true),
    &OrderObjectPosition::default(),
    default_field_settings_by_layout(),
  );

  let field_settings_map: HashMap<String, TestFieldSetting> =
    database_test.get_field_settings("v1", None);
  assert_eq!(field_settings_map.len(), 4);

  let field_settings_map: HashMap<String, TestFieldSetting> =
    database_test.get_field_settings("v2", None);
  assert_eq!(field_settings_map.len(), 4);
}

#[tokio::test]
async fn remove_field_remove_field_setting_test() {
  let database_id = uuid::Uuid::new_v4();
  let mut database_test = create_database_with_default_data(1, &database_id.to_string()).await;
  let params = CreateViewParams {
    database_id: database_id.to_string(),
    view_id: "v2".to_string(),
    field_settings: field_settings_for_default_database(),
    ..Default::default()
  };
  database_test.create_linked_view(params).unwrap();

  // Delete a field
  database_test.delete_field("f3");

  let field_settings_map: HashMap<String, TestFieldSetting> =
    database_test.get_field_settings("v1", None);
  assert_eq!(field_settings_map.len(), 2);

  let field_settings_map: HashMap<String, TestFieldSetting> =
    database_test.get_field_settings("v2", None);
  assert_eq!(field_settings_map.len(), 2);
}

#[tokio::test]
async fn update_field_setting_for_some_fields_test() {
  let database_id = uuid::Uuid::new_v4();
  let mut database_test = create_database_with_default_data(1, &database_id.to_string()).await;
  let field_settings = TestFieldSetting {
    width: 100,
    visibility: 1,
  };
  let params = CreateViewParams {
    database_id: "1".to_string(),
    view_id: "v2".to_string(),
    field_settings: field_settings_for_default_database(),
    ..Default::default()
  };
  database_test.create_linked_view(params).unwrap();

  // Update field settings for one field
  database_test.update_field_settings("v1", Some(vec!["f1".to_string()]), field_settings.clone());

  // on v1, the field settings for f1 should change
  let field_settings_map: HashMap<String, TestFieldSetting> =
    database_test.get_field_settings("v1", None);
  let test_field_settings = field_settings_map.get("f1").unwrap();
  assert_eq!(test_field_settings.visibility, 1);

  // on v2, the field settings for f1 should stay the same
  let field_settings_map: HashMap<String, TestFieldSetting> =
    database_test.get_field_settings("v2", None);
  let test_field_settings = field_settings_map.get("f1").unwrap();
  assert_eq!(test_field_settings.to_owned().visibility, 0);

  // Update field settings for all fields
  database_test.update_field_settings("v1", None, field_settings);
}

#[tokio::test]
async fn update_field_setting_test() {
  let database_id = uuid::Uuid::new_v4();
  let mut database_test = create_database_with_default_data(1, &database_id.to_string()).await;
  let field_settings = TestFieldSetting {
    width: 100,
    visibility: 1,
  };

  // Update field settings for one field
  database_test.update_field_settings("v1", Some(vec!["f1".to_string()]), field_settings);

  // the field settings for f1 should change
  let field_settings_map: HashMap<String, TestFieldSetting> =
    database_test.get_field_settings("v1", None);
  let test_field_settings = field_settings_map.get("f1").unwrap();
  assert_eq!(test_field_settings.to_owned().visibility, 1);
  assert_eq!(test_field_settings.to_owned().width, 100);
}

#[tokio::test]
async fn duplicate_view_duplicates_field_settings_test() {
  let database_id = uuid::Uuid::new_v4();
  let mut database_test = create_database_with_default_data(1, &database_id.to_string()).await;
  let field_settings = TestFieldSetting {
    width: 100,
    visibility: 1,
  };

  // Update field settings for one field
  database_test.update_field_settings("v1", Some(vec!["f1".to_string()]), field_settings);

  // the field settings for f1 should change
  let field_settings_map: HashMap<String, TestFieldSetting> =
    database_test.get_field_settings("v1", None);
  let test_field_settings = field_settings_map.get("f1").unwrap();
  assert_eq!(test_field_settings.visibility, 1);

  // duplicate view v1
  let duplicate_view = database_test.duplicate_linked_view("v1").unwrap();

  // on the duplicate view, the field settings for f1 should be the same
  let field_settings_map: HashMap<String, TestFieldSetting> =
    database_test.get_field_settings(&duplicate_view.id, None);
  let test_field_settings = field_settings_map.get("f1").unwrap();
  assert_eq!(field_settings_map.len(), 3);
  assert_eq!(test_field_settings.visibility, 1);
}

#[tokio::test]
async fn new_view_requires_deps_field_test() {
  let database_id = uuid::Uuid::new_v4();
  let mut database_test = create_database_with_default_data(1, &database_id.to_string()).await;
  let deps_field = Field::new("f4".to_string(), "date".to_string(), 3, false);
  let params = CreateViewParams {
    database_id: database_id.to_string(),
    view_id: "v2".to_string(),
    layout: DatabaseLayout::Calendar,
    field_settings: field_settings_for_default_database(),
    deps_fields: vec![deps_field],
    deps_field_setting: vec![default_field_settings_by_layout()],
    ..Default::default()
  };
  database_test.create_linked_view(params).unwrap();

  // on v1, the new field should be created
  let field_settings_map: HashMap<String, TestFieldSetting> =
    database_test.get_field_settings("v1", None);
  let test_field_settings = field_settings_map.get("f4").unwrap();
  assert_eq!(field_settings_map.len(), 4);
  assert_eq!(test_field_settings.visibility, 0);

  // on v2, the new field should also be created and is invisible
  let field_settings_map: HashMap<String, TestFieldSetting> =
    database_test.get_field_settings("v2", None);
  let test_field_settings = field_settings_map.get("f4").unwrap();
  assert_eq!(field_settings_map.len(), 4);
  assert_eq!(test_field_settings.visibility, 0);
}
