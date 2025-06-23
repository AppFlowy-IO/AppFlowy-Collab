#![allow(clippy::upper_case_acronyms)]
use crate::database::{DatabaseData, gen_database_id, gen_database_view_id, gen_row_id, timestamp};
use crate::error::DatabaseError;
use crate::fields::checkbox_type_option::CheckboxTypeOption;
use crate::fields::checklist_type_option::ChecklistTypeOption;
use crate::fields::date_type_option::{DateTypeOption, TimeTypeOption};
use crate::fields::media_type_option::MediaTypeOption;
use crate::fields::number_type_option::NumberTypeOption;
use crate::fields::relation_type_option::RelationTypeOption;
use crate::fields::select_type_option::{MultiSelectTypeOption, SingleSelectTypeOption};
use crate::fields::summary_type_option::SummarizationTypeOption;
use crate::fields::text_type_option::RichTextTypeOption;
use crate::fields::timestamp_type_option::TimestampTypeOption;
use crate::fields::translate_type_option::TranslateTypeOption;
use crate::fields::url_type_option::URLTypeOption;
use crate::fields::{Field, TypeOptionData};
use crate::rows::CreateRowParams;
use crate::views::{
  DatabaseLayout, FieldOrder, FieldSettingsByFieldIdMap, FieldSettingsMap, FilterMap,
  GroupSettingMap, LayoutSetting, LayoutSettings, OrderObjectPosition, RowOrder, SortMap,
};

use collab::entity::EncodedCollab;
use collab_entity::CollabType;
use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use tracing::error;
use uuid::Uuid;
use yrs::{Any, Out};

pub struct EncodedDatabase {
  pub encoded_database_collab: EncodedCollabInfo,
  pub encoded_row_collabs: Vec<EncodedCollabInfo>,
}

impl EncodedDatabase {
  pub fn into_collabs(self) -> Vec<EncodedCollabInfo> {
    let mut collabs = vec![self.encoded_database_collab];
    collabs.extend(self.encoded_row_collabs);
    collabs
  }
}

pub struct EncodedCollabInfo {
  pub object_id: Uuid,
  pub collab_type: CollabType,
  pub encoded_collab: EncodedCollab,
}

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
  #[serde(default)]
  pub is_inline: bool,
}

impl DatabaseView {
  pub fn new(database_id: String, view_id: String, name: String, layout: DatabaseLayout) -> Self {
    let timestamp = timestamp();
    Self {
      id: view_id,
      database_id,
      name,
      layout,
      created_at: timestamp,
      modified_at: timestamp,
      ..Default::default()
    }
  }
}

/// A meta of [DatabaseView]
#[derive(Debug, Clone)]
pub struct DatabaseViewMeta {
  pub id: String,
  pub name: String,
  pub is_inline: bool,
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
  pub fields: Vec<Field>,
  pub rows: Vec<CreateRowParams>,
  pub views: Vec<CreateViewParams>,
}

impl CreateDatabaseParams {
  /// This function creates a converts a `CreateDatabaseParams` that can be used to create a new
  /// database with the same data inside the given `DatabaseData` struct containing all the
  /// data of a database. The internal `database_id`, the database views' `view_id`s and the rows'
  /// `row_id`s will all be regenerated.
  pub fn from_database_data(
    data: DatabaseData,
    database_view_id: &str,
    new_database_view_id: &str,
  ) -> Self {
    let database_id = gen_database_id();
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
      .map(|view| CreateViewParams {
        database_id: database_id.clone(),
        view_id: if view.id == database_view_id {
          new_database_view_id.to_string()
        } else {
          gen_database_view_id()
        },
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
      })
      .collect();

    Self {
      database_id,
      rows: create_row_params,
      fields: data.fields,
      views: create_view_params,
    }
  }
}

#[derive(Clone, Copy, Debug, Hash, Eq, PartialEq, Serialize_repr, Deserialize_repr)]
#[repr(u8)]
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
  Media = 14,
}

impl FieldType {
  pub fn type_id(&self) -> String {
    (*self as i64).to_string()
  }
}

impl From<FieldType> for i64 {
  fn from(field_type: FieldType) -> Self {
    field_type as i64
  }
}

impl From<&FieldType> for i64 {
  fn from(field_type: &FieldType) -> Self {
    *field_type as i64
  }
}

impl TryFrom<yrs::Out> for FieldType {
  type Error = yrs::Out;

  fn try_from(value: yrs::Out) -> Result<Self, Self::Error> {
    match value {
      Out::Any(Any::BigInt(field_type)) => Ok(FieldType::from(field_type)),
      _ => Err(value),
    }
  }
}

impl Display for FieldType {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    let value: i64 = (*self).into();
    f.write_fmt(format_args!("{}", value))
  }
}

impl AsRef<FieldType> for FieldType {
  fn as_ref(&self) -> &FieldType {
    self
  }
}

impl From<&FieldType> for FieldType {
  fn from(field_type: &FieldType) -> Self {
    *field_type
  }
}

impl FieldType {
  pub fn value(&self) -> i64 {
    (*self).into()
  }

  pub fn default_name(&self) -> String {
    let s = match self {
      FieldType::RichText => "Text",
      FieldType::Number => "Number",
      FieldType::DateTime => "Date",
      FieldType::SingleSelect => "Single Select",
      FieldType::MultiSelect => "Multi Select",
      FieldType::Checkbox => "Checkbox",
      FieldType::URL => "URL",
      FieldType::Checklist => "Checklist",
      FieldType::LastEditedTime => "Last modified",
      FieldType::CreatedTime => "Created time",
      FieldType::Relation => "Relation",
      FieldType::Summary => "Summarize",
      FieldType::Translate => "Translate",
      FieldType::Time => "Time",
      FieldType::Media => "Media",
    };
    s.to_string()
  }

  pub fn is_ai_field(&self) -> bool {
    matches!(self, FieldType::Summary | FieldType::Translate)
  }

  pub fn is_number(&self) -> bool {
    matches!(self, FieldType::Number)
  }

  pub fn is_text(&self) -> bool {
    matches!(self, FieldType::RichText)
  }

  pub fn is_checkbox(&self) -> bool {
    matches!(self, FieldType::Checkbox)
  }

  pub fn is_date(&self) -> bool {
    matches!(self, FieldType::DateTime)
  }

  pub fn is_single_select(&self) -> bool {
    matches!(self, FieldType::SingleSelect)
  }

  pub fn is_multi_select(&self) -> bool {
    matches!(self, FieldType::MultiSelect)
  }

  pub fn is_last_edited_time(&self) -> bool {
    matches!(self, FieldType::LastEditedTime)
  }

  pub fn is_created_time(&self) -> bool {
    matches!(self, FieldType::CreatedTime)
  }

  pub fn is_url(&self) -> bool {
    matches!(self, FieldType::URL)
  }

  pub fn is_select_option(&self) -> bool {
    self.is_single_select() || self.is_multi_select()
  }

  pub fn is_checklist(&self) -> bool {
    matches!(self, FieldType::Checklist)
  }

  pub fn is_relation(&self) -> bool {
    matches!(self, FieldType::Relation)
  }

  pub fn is_time(&self) -> bool {
    matches!(self, FieldType::Time)
  }

  pub fn is_media(&self) -> bool {
    matches!(self, FieldType::Media)
  }

  pub fn can_be_group(&self) -> bool {
    self.is_select_option() || self.is_checkbox() || self.is_url()
  }

  pub fn is_auto_update(&self) -> bool {
    self.is_last_edited_time()
  }
}

impl From<i64> for FieldType {
  fn from(index: i64) -> Self {
    match index {
      0 => FieldType::RichText,
      1 => FieldType::Number,
      2 => FieldType::DateTime,
      3 => FieldType::SingleSelect,
      4 => FieldType::MultiSelect,
      5 => FieldType::Checkbox,
      6 => FieldType::URL,
      7 => FieldType::Checklist,
      8 => FieldType::LastEditedTime,
      9 => FieldType::CreatedTime,
      10 => FieldType::Relation,
      11 => FieldType::Summary,
      12 => FieldType::Translate,
      13 => FieldType::Time,
      14 => FieldType::Media,
      _ => {
        error!("Unknown field type: {}, fallback to text", index);
        FieldType::RichText
      },
    }
  }
}

pub fn default_type_option_data_from_type(field_type: FieldType) -> TypeOptionData {
  match field_type {
    FieldType::RichText => RichTextTypeOption.into(),
    FieldType::Number => NumberTypeOption::default().into(),
    FieldType::DateTime => DateTypeOption::default().into(),
    FieldType::LastEditedTime | FieldType::CreatedTime => TimestampTypeOption {
      field_type: field_type.into(),
      ..Default::default()
    }
    .into(),
    FieldType::SingleSelect => SingleSelectTypeOption::default().into(),
    FieldType::MultiSelect => MultiSelectTypeOption::default().into(),
    FieldType::Checkbox => CheckboxTypeOption.into(),
    FieldType::URL => URLTypeOption::default().into(),
    FieldType::Time => TimeTypeOption.into(),
    FieldType::Media => MediaTypeOption::default().into(),
    FieldType::Checklist => ChecklistTypeOption.into(),
    FieldType::Relation => RelationTypeOption::default().into(),
    FieldType::Summary => SummarizationTypeOption::default().into(),
    FieldType::Translate => TranslateTypeOption::default().into(),
  }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize_repr, Deserialize_repr)]
#[repr(u8)]
pub enum FileUploadType {
  #[default]
  LocalFile = 0,
  NetworkFile = 1,
  CloudFile = 2,
}
