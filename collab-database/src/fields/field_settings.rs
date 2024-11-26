use std::collections::HashMap;

use collab::util::AnyMapExt;
use strum::IntoEnumIterator;
use yrs::Any;

use crate::views::{
  DatabaseLayout, FieldSettingsByFieldIdMap, FieldSettingsMap, FieldSettingsMapBuilder,
};

use super::Field;

#[repr(u8)]
#[derive(Debug, Default, Clone, Eq, PartialEq)]
pub enum FieldVisibility {
  #[default]
  AlwaysShown = 0,
  HideWhenEmpty = 1,
  AlwaysHidden = 2,
}

macro_rules! impl_into_field_visibility {
  ($target: ident) => {
    impl std::convert::From<$target> for FieldVisibility {
      fn from(ty: $target) -> Self {
        match ty {
          0 => FieldVisibility::AlwaysShown,
          1 => FieldVisibility::HideWhenEmpty,
          2 => FieldVisibility::AlwaysHidden,
          _ => {
            tracing::error!("🔴Can't parse FieldVisibility from value: {}", ty);
            FieldVisibility::AlwaysShown
          },
        }
      }
    }
  };
}

impl_into_field_visibility!(i64);
impl_into_field_visibility!(u8);

impl From<FieldVisibility> for i64 {
  fn from(value: FieldVisibility) -> Self {
    (value as u8) as i64
  }
}

/// Stores the field settings for a single field
#[derive(Debug, Clone)]
pub struct FieldSettings {
  pub field_id: String,
  pub visibility: FieldVisibility,
  pub width: i32,
  pub wrap_cell_content: bool,
}

/// Helper struct to create a new field setting
pub struct FieldSettingsBuilder {
  inner: FieldSettings,
}

impl FieldSettingsBuilder {
  pub fn new(field_id: &str) -> Self {
    let field_settings = FieldSettings {
      field_id: field_id.to_string(),
      visibility: FieldVisibility::AlwaysShown,
      width: DEFAULT_WIDTH,
      wrap_cell_content: true,
    };

    Self {
      inner: field_settings,
    }
  }

  pub fn visibility(mut self, visibility: FieldVisibility) -> Self {
    self.inner.visibility = visibility;
    self
  }

  pub fn width(mut self, width: i32) -> Self {
    self.inner.width = width;
    self
  }

  pub fn build(self) -> FieldSettings {
    self.inner
  }
}

pub const VISIBILITY: &str = "visibility";
pub const WIDTH: &str = "width";
pub const DEFAULT_WIDTH: i32 = 150;
pub const WRAP_CELL_CONTENT: &str = "wrap";

pub fn default_field_visibility(layout_type: DatabaseLayout) -> FieldVisibility {
  match layout_type {
    DatabaseLayout::Grid => FieldVisibility::AlwaysShown,
    DatabaseLayout::Board => FieldVisibility::HideWhenEmpty,
    DatabaseLayout::Calendar => FieldVisibility::HideWhenEmpty,
  }
}

pub fn default_field_settings_for_fields(
  fields: &[Field],
  layout_type: DatabaseLayout,
) -> FieldSettingsByFieldIdMap {
  fields
    .iter()
    .map(|field| {
      let field_settings = field_settings_for_field(layout_type, field);
      (field.id.clone(), field_settings)
    })
    .collect::<HashMap<_, _>>()
    .into()
}

pub fn field_settings_for_field(
  database_layout: DatabaseLayout,
  field: &Field,
) -> FieldSettingsMap {
  let visibility = if field.is_primary {
    FieldVisibility::AlwaysShown
  } else {
    default_field_visibility(database_layout)
  };

  FieldSettingsBuilder::new(&field.id)
    .visibility(visibility)
    .build()
    .into()
}

pub fn default_field_settings_by_layout_map() -> HashMap<DatabaseLayout, FieldSettingsMap> {
  let mut map = HashMap::new();
  for layout_ty in DatabaseLayout::iter() {
    let visibility = default_field_visibility(layout_ty);
    let field_settings =
      FieldSettingsMapBuilder::from([(VISIBILITY.into(), Any::BigInt(i64::from(visibility)))]);
    map.insert(layout_ty, field_settings);
  }

  map
}

impl FieldSettings {
  pub fn from_any_map(
    field_id: &str,
    layout_type: DatabaseLayout,
    field_settings: &FieldSettingsMap,
  ) -> Self {
    let visibility = field_settings
      .get_as::<i64>(VISIBILITY)
      .map(Into::into)
      .unwrap_or_else(|| default_field_visibility(layout_type));
    let width = field_settings.get_as::<i32>(WIDTH).unwrap_or(DEFAULT_WIDTH);
    let wrap_cell_content: bool = field_settings.get_as(WRAP_CELL_CONTENT).unwrap_or(true);

    Self {
      field_id: field_id.to_string(),
      visibility,
      width,
      wrap_cell_content,
    }
  }
}

impl From<FieldSettings> for FieldSettingsMap {
  fn from(field_settings: FieldSettings) -> Self {
    FieldSettingsMapBuilder::from([
      (
        VISIBILITY.into(),
        Any::BigInt(i64::from(field_settings.visibility)),
      ),
      (WIDTH.into(), Any::BigInt(field_settings.width as i64)),
      (
        WRAP_CELL_CONTENT.into(),
        Any::Bool(field_settings.wrap_cell_content),
      ),
    ])
  }
}
