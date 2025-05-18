use collab_folder::{
  SPACE_CREATED_AT_KEY, SPACE_ICON_COLOR_KEY, SPACE_ICON_KEY, SPACE_IS_SPACE_KEY,
  SPACE_PERMISSION_KEY, SpacePermission, hierarchy_builder::ViewExtraBuilder, timestamp,
};
use serde_json::json;

#[test]
fn create_public_space_test() {
  let builder = ViewExtraBuilder::new();
  let timestamp = timestamp();
  let space_info = builder
    .is_space(true)
    .with_space_permission(SpacePermission::PublicToAll)
    .with_space_icon(Some("interface_essential/home-3"))
    .with_space_icon_color(Some("0xFFA34AFD"))
    .build();
  let space_info_json = serde_json::to_value(space_info).unwrap();
  assert_json_diff::assert_json_eq!(
    space_info_json,
    json!({
      SPACE_IS_SPACE_KEY: true,
      SPACE_PERMISSION_KEY: 0,
      SPACE_ICON_KEY: "interface_essential/home-3",
      SPACE_ICON_COLOR_KEY: "0xFFA34AFD",
      SPACE_CREATED_AT_KEY: timestamp
    }),
  );
}

#[test]
fn create_private_space_test() {
  let builder = ViewExtraBuilder::new();
  let timestamp = timestamp();
  let space_info = builder
    .is_space(true)
    .with_space_permission(SpacePermission::Private)
    .with_space_icon(Some("interface_essential/lock"))
    .with_space_icon_color(Some("0xFF4A4AFD"))
    .build();
  let space_info_json = serde_json::to_value(space_info).unwrap();
  assert_json_diff::assert_json_eq!(
    space_info_json,
    json!({
      SPACE_IS_SPACE_KEY: true,
      SPACE_PERMISSION_KEY: 1,
      SPACE_ICON_KEY: "interface_essential/lock",
      SPACE_ICON_COLOR_KEY: "0xFF4A4AFD",
      SPACE_CREATED_AT_KEY: timestamp
    }),
  );
}

#[test]
fn create_space_without_icon_and_color_test() {
  let builder = ViewExtraBuilder::new();
  let timestamp = timestamp();
  let space_info = builder
    .is_space(true)
    .with_space_permission(SpacePermission::PublicToAll)
    .build();
  let space_info_json = serde_json::to_value(space_info).unwrap();
  assert_json_diff::assert_json_eq!(
    space_info_json,
    json!({
      SPACE_IS_SPACE_KEY: true,
      SPACE_PERMISSION_KEY: 0,
      SPACE_CREATED_AT_KEY: timestamp
    }),
  );
}

#[test]
fn create_non_space_test() {
  let builder = ViewExtraBuilder::new();
  let space_info = builder.build();
  let space_info_json = serde_json::to_value(space_info).unwrap();
  assert_json_diff::assert_json_eq!(space_info_json, json!({}),);
}
