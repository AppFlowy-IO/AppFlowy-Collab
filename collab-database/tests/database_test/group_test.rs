use crate::helper::{
  create_database_with_default_data, TestGroup, TestGroupSetting, CONTENT, GROUPS,
};
use collab::core::any_map::AnyMapExtension;
use collab_database::views::{CreateViewParams, DatabaseLayout};

#[test]
fn create_database_view_with_group_test() {
  let database_test = create_database_with_default_data(1, "1");
  let group_1 = TestGroupSetting {
    id: "group1".to_string(),
    field_id: "".to_string(),
    field_type: Default::default(),
    groups: vec![
      TestGroup {
        id: "group_item1".to_string(),
        name: "group item 1".to_string(),
        visible: false,
      },
      TestGroup {
        id: "group_item2".to_string(),
        name: "group item 2".to_string(),
        visible: false,
      },
    ],
    content: "".to_string(),
  };
  let group_2 = TestGroupSetting {
    id: "group2".to_string(),
    field_id: "".to_string(),
    field_type: Default::default(),
    groups: vec![],
    content: "".to_string(),
  };

  let params = CreateViewParams {
    view_id: "v1".to_string(),
    groups: vec![group_1.into(), group_2.into()],
    layout: DatabaseLayout::Grid,
    ..Default::default()
  };
  database_test.create_view(params);

  let view = database_test.views.get_view("v1").unwrap();
  assert_eq!(view.group_settings.len(), 2);
  let group_settings = view
    .group_settings
    .iter()
    .map(TestGroupSetting::from)
    .collect::<Vec<TestGroupSetting>>();

  assert_eq!(group_settings[1].id, "group2");
  assert_eq!(group_settings[0].id, "group1");
  assert_eq!(group_settings[0].groups.len(), 2);
  assert_eq!(group_settings[0].groups[0].id, "group_item1");
  assert_eq!(group_settings[0].groups[1].id, "group_item2");
}

#[test]
fn create_database_view_with_group_test2() {
  let database_test = create_database_with_default_data(1, "1");
  let params = CreateViewParams {
    view_id: "v1".to_string(),
    ..Default::default()
  };
  database_test.create_view(params);
  let group_setting = TestGroupSetting {
    id: "g1".to_string(),
    field_id: "".to_string(),
    field_type: Default::default(),
    groups: vec![
      TestGroup {
        id: "group_item1".to_string(),
        name: "group item 1".to_string(),
        visible: false,
      },
      TestGroup {
        id: "group_item2".to_string(),
        name: "group item 2".to_string(),
        visible: false,
      },
    ],
    content: "".to_string(),
  };
  database_test.add_group_setting("v1", group_setting);

  let view = database_test.views.get_view("v1").unwrap();
  assert_eq!(view.group_settings.len(), 1);
  let group_settings = view
    .group_settings
    .iter()
    .map(TestGroupSetting::from)
    .collect::<Vec<TestGroupSetting>>();

  assert_eq!(group_settings[0].id, "g1");
  assert_eq!(group_settings[0].groups.len(), 2);
  assert_eq!(group_settings[0].groups[0].id, "group_item1");
  assert_eq!(group_settings[0].groups[1].id, "group_item2");
}

#[test]
fn override_database_view_group_test() {
  let database_test = create_database_with_default_data(1, "1");
  let params = CreateViewParams {
    view_id: "v1".to_string(),
    ..Default::default()
  };
  database_test.create_view(params);
  let group_setting = TestGroupSetting {
    id: "g1".to_string(),
    groups: vec![TestGroup {
      id: "group_item1".to_string(),
      name: "group item 1".to_string(),
      visible: false,
    }],
    ..Default::default()
  };
  database_test.add_group_setting("v1", group_setting);
  database_test.update_group_setting("v1", "g1", |object| {
    object.insert_str_value(CONTENT, "hello world".to_string());
    object.insert_any_maps(
      GROUPS,
      vec![TestGroup {
        id: "group_item2".to_string(),
        name: "group item 2".to_string(),
        visible: false,
      }],
    );
  });

  let view = database_test.views.get_view("v1").unwrap();
  assert_eq!(view.group_settings.len(), 1);
  let group_settings = view
    .group_settings
    .iter()
    .map(TestGroupSetting::from)
    .collect::<Vec<TestGroupSetting>>();

  assert_eq!(group_settings[0].content, "hello world");
  assert_eq!(group_settings[0].groups.len(), 1);
  assert_eq!(group_settings[0].groups[0].id, "group_item2");
}
