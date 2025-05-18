use collab::preclude::Any;
use collab::util::{AnyExt, AnyMapExt};
use collab_database::entity::CreateViewParams;
use collab_database::views::{DatabaseLayout, GroupMap};

use crate::database_test::helper::{DatabaseTest, create_database_with_default_data};
use crate::helper::{CONTENT, GROUPS, TestGroup, TestGroupSetting};

#[tokio::test]
async fn create_database_view_with_group_test() {
  let database_test = create_database_with_two_groups().await;
  let view = database_test.get_view("v1").unwrap();
  assert_eq!(view.group_settings.len(), 2);
  let group_settings = view
    .group_settings
    .iter()
    .map(|value| TestGroupSetting::try_from(value).unwrap())
    .collect::<Vec<TestGroupSetting>>();

  assert_eq!(group_settings[1].id, "g2");
  assert_eq!(group_settings[0].id, "g1");
  assert_eq!(group_settings[0].groups.len(), 2);
  assert_eq!(group_settings[0].groups[0].id, "group_item1");
  assert_eq!(group_settings[0].groups[1].id, "group_item2");
}

#[tokio::test]
async fn create_database_view_with_group_test2() {
  let database_id = uuid::Uuid::new_v4();
  let mut database_test = create_database_with_default_data(1, &database_id.to_string()).await;
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
  database_test.insert_group_setting("v1", group_setting);

  let view = database_test.get_view("v1").unwrap();
  assert_eq!(view.group_settings.len(), 1);
  let group_settings = view
    .group_settings
    .iter()
    .map(|value| TestGroupSetting::try_from(value).unwrap())
    .collect::<Vec<TestGroupSetting>>();

  assert_eq!(group_settings[0].id, "g1");
  assert_eq!(group_settings[0].groups.len(), 2);
  assert_eq!(group_settings[0].groups[0].id, "group_item1");
  assert_eq!(group_settings[0].groups[1].id, "group_item2");
}

#[tokio::test]
async fn get_single_database_group_test() {
  let database_id = uuid::Uuid::new_v4();
  let mut database_test = create_database_with_default_data(1, &database_id.to_string()).await;
  let group_setting = TestGroupSetting {
    id: "g1".to_string(),
    field_id: "f1".to_string(),
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
    content: "test group".to_string(),
  };
  database_test.insert_group_setting("v1", group_setting);
  let settings = database_test.get_all_group_setting::<TestGroupSetting>("v1");
  assert_eq!(settings.len(), 1);
  assert_eq!(settings[0].id, "g1");
  assert_eq!(settings[0].content, "test group");
  assert_eq!(settings[0].groups.len(), 2);
  assert_eq!(settings[0].groups[0].id, "group_item1");
  assert_eq!(settings[0].groups[1].id, "group_item2");
}

#[tokio::test]
async fn get_multiple_database_group_test() {
  let database_id = uuid::Uuid::new_v4();
  let mut database_test = create_database_with_default_data(1, &database_id.to_string()).await;
  let group_setting_1 = TestGroupSetting {
    id: "g1".to_string(),
    field_id: "f1".to_string(),
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
    content: "test group".to_string(),
  };
  let group_setting_2 = TestGroupSetting {
    id: "g2".to_string(),
    field_id: "f1".to_string(),
    field_type: Default::default(),
    groups: vec![],
    content: "test group 2".to_string(),
  };
  database_test.insert_group_setting("v1", group_setting_1);
  database_test.insert_group_setting("v1", group_setting_2);

  let settings = database_test.get_all_group_setting::<TestGroupSetting>("v1");
  assert_eq!(settings.len(), 2);
  assert_eq!(settings[1].id, "g2");
  assert_eq!(settings[1].content, "test group 2");
  assert_eq!(settings[1].groups.len(), 0);
}

#[tokio::test]
async fn extend_database_view_group_test() {
  let mut database_test = create_database_with_two_groups().await;
  database_test.update_group_setting("v1", "g1", |object| {
    object.insert(CONTENT.into(), "hello world".into());
    let mut groups = object
      .remove(GROUPS)
      .and_then(|any| any.into_array())
      .unwrap_or_default();
    groups.push(Any::from(GroupMap::from(TestGroup {
      id: "group_item3".to_string(),
      name: "group item 3".to_string(),
      visible: false,
    })));
    object.insert(GROUPS.into(), Any::from(groups));
  });

  let view = database_test.get_view("v1").unwrap();
  assert_eq!(view.group_settings.len(), 2);
  let group_settings = view
    .group_settings
    .iter()
    .map(|value| TestGroupSetting::try_from(value).unwrap())
    .collect::<Vec<TestGroupSetting>>();

  assert_eq!(group_settings[0].content, "hello world");
  assert_eq!(group_settings[0].groups.len(), 3);
  assert_eq!(group_settings[0].groups[0].id, "group_item1");
  assert_eq!(group_settings[0].groups[1].id, "group_item2");
  assert_eq!(group_settings[0].groups[2].id, "group_item3");
}

#[tokio::test]
async fn remove_database_view_group_test() {
  let mut database_test = create_database_with_two_groups().await;
  database_test.update_group_setting("v1", "g1", |object| {
    let mut groups = object
      .remove(GROUPS)
      .and_then(|any| any.into_array())
      .unwrap_or_default();
    let index = groups
      .iter()
      .position(|group| group.get_as::<String>("id").as_deref() == Some("group_item1"))
      .unwrap();
    groups.remove(index);
    object.insert(GROUPS.into(), groups.into());
  });

  let view = database_test.get_view("v1").unwrap();
  let group_settings = view
    .group_settings
    .iter()
    .map(|value| TestGroupSetting::try_from(value).unwrap())
    .collect::<Vec<TestGroupSetting>>();

  assert_eq!(group_settings[0].id, "g1");
  assert_eq!(group_settings[0].groups.len(), 1);
  assert_eq!(group_settings[0].groups[0].id, "group_item2");
}

async fn create_database_with_two_groups() -> DatabaseTest {
  let database_id = uuid::Uuid::new_v4();
  let mut database_test = create_database_with_default_data(1, &database_id.to_string()).await;
  let group_1 = TestGroupSetting {
    id: "g1".to_string(),
    field_id: "f1".to_string(),
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
    id: "g2".to_string(),
    field_id: "f2".to_string(),
    field_type: Default::default(),
    groups: vec![],
    content: "".to_string(),
  };

  let params = CreateViewParams {
    database_id: "1".to_string(),
    view_id: "v1".to_string(),
    group_settings: vec![group_1.into(), group_2.into()],
    layout: DatabaseLayout::Grid,
    ..Default::default()
  };
  database_test.create_linked_view(params).unwrap();
  database_test
}
