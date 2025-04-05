use serde::{Deserialize, Serialize};

use crate::timestamp;

pub const SPACE_IS_SPACE_KEY: &str = "is_space";
pub const SPACE_PERMISSION_KEY: &str = "space_permission";
pub const SPACE_ICON_KEY: &str = "space_icon";
pub const SPACE_ICON_COLOR_KEY: &str = "space_icon_color";
pub const SPACE_CREATED_AT_KEY: &str = "space_created_at";

/// Represents the space info of a view
///
/// Two view types are supported:
///
/// - Space view: A view associated with a space info. Parent view that can contain normal views.
///   Child views inherit the space's permissions.
///
/// - Normal view: Cannot contain space views and has no direct permission controls.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SpaceInfo {
  /// Whether the view is a space view.
  pub is_space: bool,

  /// The permission of the space view.
  ///
  /// If the space_permission is none, the space view will use the SpacePermission::PublicToAll.
  #[serde(default)]
  pub space_permission: SpacePermission,

  /// The created time of the space view.
  pub space_created_at: i64,

  /// The space icon.
  ///
  /// If the space_icon is none, the space view will use the default icon.
  pub space_icon: Option<String>,

  /// The space icon color.
  ///
  /// If the space_icon_color is none, the space view will use the default icon color.
  /// The value should be a valid hex color code: 0xFFA34AFD
  pub space_icon_color: Option<String>,
}

impl Default for SpaceInfo {
  /// Default space info is a public space
  ///
  /// The permission is public to all
  /// The created time is the current timestamp
  fn default() -> Self {
    Self {
      is_space: true,
      space_permission: SpacePermission::PublicToAll,
      space_created_at: timestamp(),
      space_icon: None,
      space_icon_color: None,
    }
  }
}

#[derive(
  Debug, Clone, Default, serde_repr::Serialize_repr, serde_repr::Deserialize_repr, PartialEq, Eq,
)]
#[repr(u8)]
pub enum SpacePermission {
  #[default]
  PublicToAll = 0,
  Private = 1,
}
