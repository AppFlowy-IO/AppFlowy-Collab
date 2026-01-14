use serde::Deserialize;
use serde_repr::{Deserialize_repr, Serialize_repr};
use yrs::{Any, encoding::serde::from_any};

use super::LayoutSetting;

#[derive(Debug, Clone, Deserialize)]
pub struct CalendarLayoutSetting {
  #[serde(default)]
  pub layout_ty: CalendarLayout,
  #[serde(default, rename(deserialize = "first_day_of_week_v2"))]
  pub first_day_of_week: Option<i32>,
  #[serde(default)]
  pub show_weekends: bool,
  #[serde(default)]
  pub show_week_numbers: bool,
  #[serde(default)]
  pub field_id: String,
}

impl From<LayoutSetting> for CalendarLayoutSetting {
  fn from(setting: LayoutSetting) -> Self {
    from_any(&Any::from(setting)).unwrap()
  }
}

impl From<CalendarLayoutSetting> for LayoutSetting {
  fn from(setting: CalendarLayoutSetting) -> Self {
    let mut result = LayoutSetting::from([
      ("layout_ty".into(), Any::BigInt(setting.layout_ty.value())),
      (
        "show_week_numbers".into(),
        Any::Bool(setting.show_week_numbers),
      ),
      ("show_weekends".into(), Any::Bool(setting.show_weekends)),
      ("field_id".into(), setting.field_id.into()),
    ]);

    if let Some(first_day_of_week) = setting.first_day_of_week {
      result.insert(
        "first_day_of_week_v2".to_string(),
        Any::BigInt(first_day_of_week as i64),
      );
    }

    result
  }
}

impl CalendarLayoutSetting {
  pub fn new(field_id: String) -> Self {
    CalendarLayoutSetting {
      layout_ty: CalendarLayout::default(),
      first_day_of_week: None,
      show_weekends: DEFAULT_SHOW_WEEKENDS,
      show_week_numbers: DEFAULT_SHOW_WEEK_NUMBERS,
      field_id,
    }
  }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Default, Serialize_repr, Deserialize_repr)]
#[repr(u8)]
pub enum CalendarLayout {
  #[default]
  Month = 0,
  Week = 1,
  Day = 2,
}

impl From<i64> for CalendarLayout {
  fn from(value: i64) -> Self {
    match value {
      0 => CalendarLayout::Month,
      1 => CalendarLayout::Week,
      2 => CalendarLayout::Day,
      _ => CalendarLayout::Month,
    }
  }
}

impl CalendarLayout {
  pub fn value(&self) -> i64 {
    *self as i64
  }
}

pub const DEFAULT_SHOW_WEEKENDS: bool = true;
pub const DEFAULT_SHOW_WEEK_NUMBERS: bool = true;

#[derive(Debug, Clone, Default, Deserialize)]
pub struct BoardLayoutSetting {
  #[serde(default)]
  pub hide_ungrouped_column: bool,
  #[serde(default)]
  pub collapse_hidden_groups: bool,
}

impl BoardLayoutSetting {
  pub fn new() -> Self {
    Self {
      hide_ungrouped_column: false,
      collapse_hidden_groups: true,
    }
  }
}

impl From<LayoutSetting> for BoardLayoutSetting {
  fn from(setting: LayoutSetting) -> Self {
    from_any(&Any::from(setting)).unwrap()
  }
}

impl From<BoardLayoutSetting> for LayoutSetting {
  fn from(setting: BoardLayoutSetting) -> Self {
    LayoutSetting::from([
      (
        "hide_ungrouped_column".into(),
        setting.hide_ungrouped_column.into(),
      ),
      (
        "collapse_hidden_groups".into(),
        setting.collapse_hidden_groups.into(),
      ),
    ])
  }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ChartLayoutSetting {
  #[serde(default)]
  pub chart_type: ChartType,
  #[serde(default)]
  pub x_field_id: String,
  #[serde(default)]
  pub show_empty_values: bool,
  #[serde(default)]
  pub aggregation_type: ChartAggregationType,
  #[serde(default)]
  pub y_field_id: String,
}

impl ChartLayoutSetting {
  pub fn new() -> Self {
    Self {
      chart_type: ChartType::Bar,
      x_field_id: String::new(),
      show_empty_values: false,
      aggregation_type: ChartAggregationType::Count,
      y_field_id: String::new(),
    }
  }
}

impl From<LayoutSetting> for ChartLayoutSetting {
  fn from(setting: LayoutSetting) -> Self {
    from_any(&Any::from(setting)).unwrap_or_default()
  }
}

impl From<ChartLayoutSetting> for LayoutSetting {
  fn from(setting: ChartLayoutSetting) -> Self {
    LayoutSetting::from([
      ("chart_type".into(), Any::BigInt(setting.chart_type.value())),
      ("x_field_id".into(), setting.x_field_id.into()),
      ("show_empty_values".into(), setting.show_empty_values.into()),
      (
        "aggregation_type".into(),
        Any::BigInt(setting.aggregation_type.value()),
      ),
      ("y_field_id".into(), setting.y_field_id.into()),
    ])
  }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Default, Serialize_repr, Deserialize_repr)]
#[repr(u8)]
pub enum ChartType {
  #[default]
  Bar = 0,
  Line = 1,
  HorizontalBar = 2,
  Donut = 3,
}

impl From<i64> for ChartType {
  fn from(value: i64) -> Self {
    match value {
      0 => ChartType::Bar,
      1 => ChartType::Line,
      2 => ChartType::HorizontalBar,
      3 => ChartType::Donut,
      _ => ChartType::Bar,
    }
  }
}

impl ChartType {
  pub fn value(&self) -> i64 {
    *self as i64
  }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Default, Serialize_repr, Deserialize_repr)]
#[repr(u8)]
pub enum ChartAggregationType {
  #[default]
  Count = 0,
  Sum = 1,
  Average = 2,
  Min = 3,
  Max = 4,
}

impl From<i64> for ChartAggregationType {
  fn from(value: i64) -> Self {
    match value {
      0 => ChartAggregationType::Count,
      1 => ChartAggregationType::Sum,
      2 => ChartAggregationType::Average,
      3 => ChartAggregationType::Min,
      4 => ChartAggregationType::Max,
      _ => ChartAggregationType::Count,
    }
  }
}

impl ChartAggregationType {
  pub fn value(&self) -> i64 {
    *self as i64
  }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Default, Serialize_repr, Deserialize_repr)]
#[repr(u8)]
pub enum GalleryCardSize {
  Small = 0,
  #[default]
  Medium = 1,
  Large = 2,
}

impl From<i64> for GalleryCardSize {
  fn from(value: i64) -> Self {
    match value {
      0 => GalleryCardSize::Small,
      1 => GalleryCardSize::Medium,
      2 => GalleryCardSize::Large,
      _ => GalleryCardSize::Medium,
    }
  }
}

impl GalleryCardSize {
  pub fn value(&self) -> i64 {
    *self as i64
  }
}

/// Layout settings for Gallery view.
///
/// These settings are intentionally small and UI-focused. “Properties shown on
/// card” behavior is primarily driven by per-view `FieldVisibility` settings.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct GalleryLayoutSetting {
  /// Whether to show cover images on cards.
  #[serde(default = "default_true")]
  pub show_cover: bool,

  /// Whether to fit the entire image in the card preview area (no cropping).
  /// When false, images are cropped to fill the preview.
  #[serde(default)]
  pub fit_image: bool,

  /// Card size (affects card width and cover height in the UI).
  #[serde(default)]
  pub card_size: GalleryCardSize,

  /// Card width in pixels (0 = auto).
  #[serde(default)]
  pub card_width: i32,
}

impl GalleryLayoutSetting {
  pub fn new() -> Self {
    Self {
      show_cover: true,
      fit_image: false,
      card_size: GalleryCardSize::Medium,
      card_width: 0,
    }
  }
}

impl From<LayoutSetting> for GalleryLayoutSetting {
  fn from(setting: LayoutSetting) -> Self {
    from_any(&Any::from(setting)).unwrap_or_default()
  }
}

impl From<GalleryLayoutSetting> for LayoutSetting {
  fn from(setting: GalleryLayoutSetting) -> Self {
    LayoutSetting::from([
      ("show_cover".into(), setting.show_cover.into()),
      ("fit_image".into(), setting.fit_image.into()),
      ("card_size".into(), Any::BigInt(setting.card_size.value())),
      ("card_width".into(), Any::BigInt(setting.card_width as i64)),
    ])
  }
}

/// Layout settings for List view.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ListLayoutSetting {
  /// Display mode for list cards
  #[serde(default)]
  pub display_mode: ListDisplayMode,
  /// Field IDs to show on cards (excluding primary field which is always shown)
  #[serde(default)]
  pub visible_field_ids: Vec<String>,
  /// Whether to show cover images on cards
  #[serde(default = "default_true")]
  pub show_cover: bool,
  /// Whether to show row icons on cards
  #[serde(default = "default_true")]
  pub show_icon: bool,
  /// Card width in pixels (0 = auto)
  #[serde(default)]
  pub card_width: i32,
  /// Optional field ID to group rows by
  #[serde(default)]
  pub group_field_id: Option<String>,
  /// Whether to show field names on cards
  #[serde(default = "default_true")]
  pub show_field_names: bool,
}

fn default_true() -> bool {
  true
}

impl ListLayoutSetting {
  pub fn new() -> Self {
    Self {
      display_mode: ListDisplayMode::Standard,
      visible_field_ids: vec![],
      show_cover: true,
      show_icon: true,
      card_width: 0,
      group_field_id: None,
      show_field_names: true,
    }
  }
}

impl From<LayoutSetting> for ListLayoutSetting {
  fn from(setting: LayoutSetting) -> Self {
    from_any(&Any::from(setting)).unwrap_or_default()
  }
}

impl From<ListLayoutSetting> for LayoutSetting {
  fn from(setting: ListLayoutSetting) -> Self {
    let mut result = LayoutSetting::from([
      (
        "display_mode".into(),
        Any::BigInt(setting.display_mode.value()),
      ),
      ("show_cover".into(), setting.show_cover.into()),
      ("show_icon".into(), setting.show_icon.into()),
      ("card_width".into(), Any::BigInt(setting.card_width as i64)),
      ("show_field_names".into(), setting.show_field_names.into()),
    ]);

    // Add visible_field_ids as array
    let field_ids: Vec<Any> = setting
      .visible_field_ids
      .into_iter()
      .map(|s| Any::String(s.into()))
      .collect();
    result.insert(
      "visible_field_ids".to_string(),
      Any::Array(field_ids.into()),
    );

    if let Some(group_field_id) = setting.group_field_id {
      result.insert("group_field_id".to_string(), group_field_id.into());
    }

    result
  }
}

/// Display mode for list view cards
#[derive(Debug, Copy, Clone, Eq, PartialEq, Default, Serialize_repr, Deserialize_repr)]
#[repr(u8)]
pub enum ListDisplayMode {
  /// Single-line cards with title only
  Compact = 0,
  /// Cards with title and 2-3 fields
  #[default]
  Standard = 1,
  /// Large cards with cover image and more fields
  Expanded = 2,
}

impl From<i64> for ListDisplayMode {
  fn from(value: i64) -> Self {
    match value {
      0 => ListDisplayMode::Compact,
      1 => ListDisplayMode::Standard,
      2 => ListDisplayMode::Expanded,
      _ => ListDisplayMode::Standard,
    }
  }
}

impl ListDisplayMode {
  pub fn value(&self) -> i64 {
    *self as i64
  }
}
