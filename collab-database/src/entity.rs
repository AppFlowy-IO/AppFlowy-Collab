#![allow(clippy::upper_case_acronyms)]
use crate::database::{
  gen_database_id, gen_database_view_id, gen_option_id, gen_row_id, timestamp, DatabaseData,
};
use crate::error::DatabaseError;
use crate::fields::Field;
use crate::rows::CreateRowParams;
use crate::views::{
  DatabaseLayout, FieldOrder, FieldSettingsByFieldIdMap, FieldSettingsMap, FilterMap,
  GroupSettingMap, LayoutSetting, LayoutSettings, OrderObjectPosition, RowOrder, SortMap,
};

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DatabaseView {
  pub id: String,
  pub database_id: String,
  pub name: String,
  pub layout: DatabaseLayout,
  pub layout_settings: LayoutSettings,
  pub filters: Vec<FilterMap>,
  pub group_settings: Vec<GroupSettingMap>,
  pub sorts: Vec<SortMap>,
  pub row_orders: Vec<RowOrder>,
  pub field_orders: Vec<FieldOrder>,
  pub field_settings: FieldSettingsByFieldIdMap,
  pub created_at: i64,
  pub modified_at: i64,
}

/// A meta of [DatabaseView]
#[derive(Debug, Clone)]
pub struct DatabaseViewMeta {
  pub id: String,
  pub name: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CreateViewParams {
  pub database_id: String,
  pub view_id: String,
  pub name: String,
  pub layout: DatabaseLayout,
  pub layout_settings: LayoutSettings,
  pub filters: Vec<FilterMap>,
  pub group_settings: Vec<GroupSettingMap>,
  pub sorts: Vec<SortMap>,
  pub field_settings: FieldSettingsByFieldIdMap,
  pub created_at: i64,
  pub modified_at: i64,

  /// When creating a view for a database, it might need to create a new field for the view.
  /// For example, if the view is calendar view, it must have a date field.
  pub deps_fields: Vec<Field>,

  /// Each new field in `deps_fields` must also have an associated FieldSettings
  /// that will be inserted into each view according to the view's layout type
  pub deps_field_setting: Vec<HashMap<DatabaseLayout, FieldSettingsMap>>,
}

impl CreateViewParams {
  pub fn take_deps_fields(
    &mut self,
  ) -> (Vec<Field>, Vec<HashMap<DatabaseLayout, FieldSettingsMap>>) {
    (
      std::mem::take(&mut self.deps_fields),
      std::mem::take(&mut self.deps_field_setting),
    )
  }
}

impl CreateViewParams {
  pub fn new(database_id: String, view_id: String, name: String, layout: DatabaseLayout) -> Self {
    Self {
      database_id,
      view_id,
      name,
      layout,
      ..Default::default()
    }
  }

  pub fn with_layout_setting(mut self, layout_setting: LayoutSetting) -> Self {
    self.layout_settings.insert(self.layout, layout_setting);
    self
  }

  pub fn with_filters(mut self, filters: Vec<FilterMap>) -> Self {
    self.filters = filters;
    self
  }

  pub fn with_groups(mut self, groups: Vec<GroupSettingMap>) -> Self {
    self.group_settings = groups;
    self
  }

  pub fn with_deps_fields(
    mut self,
    fields: Vec<Field>,
    field_settings: Vec<HashMap<DatabaseLayout, FieldSettingsMap>>,
  ) -> Self {
    self.deps_fields = fields;
    self.deps_field_setting = field_settings;
    self
  }

  pub fn with_field_settings_map(mut self, field_settings_map: FieldSettingsByFieldIdMap) -> Self {
    self.field_settings = field_settings_map;
    self
  }
}

impl From<DatabaseView> for CreateViewParams {
  fn from(view: DatabaseView) -> Self {
    Self {
      database_id: view.database_id,
      view_id: view.id,
      name: view.name,
      layout: view.layout,
      filters: view.filters,
      layout_settings: view.layout_settings,
      group_settings: view.group_settings,
      sorts: view.sorts,
      field_settings: view.field_settings,
      ..Default::default()
    }
  }
}

pub(crate) struct CreateViewParamsValidator;

impl CreateViewParamsValidator {
  pub(crate) fn validate(params: CreateViewParams) -> Result<CreateViewParams, DatabaseError> {
    if params.database_id.is_empty() {
      return Err(DatabaseError::InvalidDatabaseID("database_id is empty"));
    }

    if params.view_id.is_empty() {
      return Err(DatabaseError::InvalidViewID("view_id is empty"));
    }

    Ok(params)
  }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CreateDatabaseParams {
  pub database_id: String,
  pub inline_view_id: String,
  pub fields: Vec<Field>,
  pub rows: Vec<CreateRowParams>,
  pub views: Vec<CreateViewParams>,
}

impl CreateDatabaseParams {
  /// This function creates a converts a `CreateDatabaseParams` that can be used to create a new
  /// database with the same data inside the given `DatabaseData` struct containing all the
  /// data of a database. The internal `database_id`, the database views' `view_id`s and the rows'
  /// `row_id`s will all be regenerated.
  pub fn from_database_data(data: DatabaseData) -> Self {
    let (database_id, inline_view_id) = (gen_database_id(), gen_database_view_id());
    let timestamp = timestamp();

    let create_row_params = data
      .rows
      .into_iter()
      .map(|row| CreateRowParams {
        id: gen_row_id(),
        database_id: database_id.clone(),
        created_at: timestamp,
        modified_at: timestamp,
        cells: row.cells,
        height: row.height,
        visibility: row.visibility,
        row_position: OrderObjectPosition::End,
      })
      .collect();

    let create_view_params = data
      .views
      .into_iter()
      .map(|view| {
        let view_id = if view.id == data.inline_view_id {
          inline_view_id.clone()
        } else {
          gen_database_view_id()
        };
        CreateViewParams {
          database_id: database_id.clone(),
          view_id,
          name: view.name,
          layout: view.layout,
          layout_settings: view.layout_settings,
          filters: view.filters,
          group_settings: view.group_settings,
          sorts: view.sorts,
          field_settings: view.field_settings,
          created_at: timestamp,
          modified_at: timestamp,
          ..Default::default()
        }
      })
      .collect();

    Self {
      database_id,
      inline_view_id,
      rows: create_row_params,
      fields: data.fields,
      views: create_view_params,
    }
  }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct DateTypeOption {
  pub date_format: DateFormat,
  pub time_format: TimeFormat,
  pub timezone_id: String,
}

impl DateTypeOption {
  pub fn to_json_string(&self) -> String {
    serde_json::to_string(self).unwrap()
  }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, Serialize, Deserialize, Default)]
pub enum TimeFormat {
  TwelveHour = 0,
  #[default]
  TwentyFourHour = 1,
}
#[derive(Clone, Debug, Copy, Serialize, Deserialize, Default)]
pub enum DateFormat {
  Local = 0,
  US = 1,
  ISO = 2,
  #[default]
  Friendly = 3,
  DayMonthYear = 4,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TimestampTypeOption {
  pub date_format: DateFormat,
  pub time_format: TimeFormat,
  pub include_time: bool,
  pub field_type: FieldType,
}

impl TimestampTypeOption {
  pub fn new(field_type: FieldType, include_time: bool) -> Self {
    Self {
      date_format: DateFormat::default(),
      time_format: TimeFormat::default(),
      include_time,
      field_type,
    }
  }
}

#[derive(Clone, Debug, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub enum FieldType {
  RichText = 0,
  Number = 1,
  DateTime = 2,
  SingleSelect = 3,
  MultiSelect = 4,
  Checkbox = 5,
  URL = 6,
  Checklist = 7,
  LastEditedTime = 8,
  CreatedTime = 9,
  Relation = 10,
  Summary = 11,
  Translate = 12,
  Time = 13,
}

impl FieldType {
  pub fn type_id(&self) -> String {
    (self.clone() as i64).to_string()
  }
}

impl From<FieldType> for i64 {
  fn from(field_type: FieldType) -> Self {
    field_type as i64
  }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SelectTypeOption {
  pub options: Vec<SelectOption>,
  pub disable_color: bool,
}
impl SelectTypeOption {
  pub fn to_json_string(&self) -> String {
    serde_json::to_string(self).unwrap()
  }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SelectOption {
  pub id: String,
  pub name: String,
  pub color: SelectOptionColor,
}
impl SelectOption {
  pub fn new(name: &str) -> Self {
    SelectOption {
      id: gen_option_id(),
      name: name.to_owned(),
      color: SelectOptionColor::default(),
    }
  }

  pub fn with_color(name: &str, color: SelectOptionColor) -> Self {
    SelectOption {
      id: gen_option_id(),
      name: name.to_owned(),
      color,
    }
  }
}
#[derive(PartialEq, Eq, Serialize, Deserialize, Debug, Clone)]
#[repr(u8)]
#[derive(Default)]
pub enum SelectOptionColor {
  #[default]
  Purple = 0,
  Pink = 1,
  LightPink = 2,
  Orange = 3,
  Yellow = 4,
  Lime = 5,
  Green = 6,
  Aqua = 7,
  Blue = 8,
}

impl From<usize> for SelectOptionColor {
  fn from(index: usize) -> Self {
    match index {
      0 => SelectOptionColor::Purple,
      1 => SelectOptionColor::Pink,
      2 => SelectOptionColor::LightPink,
      3 => SelectOptionColor::Orange,
      4 => SelectOptionColor::Yellow,
      5 => SelectOptionColor::Lime,
      6 => SelectOptionColor::Green,
      7 => SelectOptionColor::Aqua,
      8 => SelectOptionColor::Blue,
      _ => SelectOptionColor::Purple,
    }
  }
}
