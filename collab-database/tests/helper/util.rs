#![allow(clippy::upper_case_acronyms)]

use std::fs::{create_dir_all, File};
use std::io::copy;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Once};

use anyhow::bail;
use collab::core::any_map::{AnyMap, AnyMapExtension};
use collab::preclude::Any;
use collab_database::fields::{TypeOptionData, TypeOptionDataBuilder};
use collab_database::rows::Cell;
use collab_database::views::{
  FieldSettingsMap, FieldSettingsMapBuilder, FilterMap, FilterMapBuilder, GroupMap,
  GroupMapBuilder, GroupSettingBuilder, GroupSettingMap, LayoutSetting, LayoutSettingBuilder,
  SortMap, SortMapBuilder,
};
use collab_plugins::CollabKVDB;
use nanoid::nanoid;
use tempfile::TempDir;
use tracing_subscriber::fmt::Subscriber;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;
use zip::ZipArchive;

#[derive(Debug, Clone)]
pub struct TestFilter {
  pub id: String,
  pub field_id: String,
  pub field_type: TestFieldType,
  pub condition: i64,
  pub content: String,
}

const FILTER_ID: &str = "id";
pub const FIELD_ID: &str = "field_id";
pub const FIELD_TYPE: &str = "ty";
pub const FILTER_CONDITION: &str = "condition";
pub const FILTER_CONTENT: &str = "content";

impl From<TestFilter> for FilterMap {
  fn from(data: TestFilter) -> Self {
    FilterMapBuilder::new()
      .insert_str_value(FILTER_ID, data.id)
      .insert_str_value(FIELD_ID, data.field_id)
      .insert_str_value(FILTER_CONTENT, data.content)
      .insert_i64_value(FIELD_TYPE, data.field_type.into())
      .insert_i64_value(FILTER_CONDITION, data.condition)
      .build()
  }
}

impl TryFrom<FilterMap> for TestFilter {
  type Error = anyhow::Error;

  fn try_from(filter: FilterMap) -> Result<Self, Self::Error> {
    match (
      filter.get_str_value(FILTER_ID),
      filter.get_str_value(FIELD_ID),
    ) {
      (Some(id), Some(field_id)) => {
        let condition = filter.get_i64_value(FILTER_CONDITION).unwrap_or(0);
        let content = filter.get_str_value(FILTER_CONTENT).unwrap_or_default();
        let field_type = filter
          .get_i64_value(FIELD_TYPE)
          .map(TestFieldType::from)
          .unwrap_or_default();
        Ok(TestFilter {
          id,
          field_id,
          field_type,
          condition,
          content,
        })
      },
      _ => {
        bail!("Invalid filter data")
      },
    }
  }
}

#[derive(Debug, Clone, Default)]
pub struct TestGroup {
  pub id: String,
  pub name: String,
  pub visible: bool,
}

impl From<GroupMap> for TestGroup {
  fn from(value: GroupMap) -> Self {
    let id = value.get_str_value("id").unwrap();
    let name = value.get_str_value("name").unwrap_or_default();
    let visible = value.get_bool_value("visible").unwrap_or_default();
    Self { id, name, visible }
  }
}

impl From<TestGroup> for GroupMap {
  fn from(group: TestGroup) -> Self {
    GroupMapBuilder::new()
      .insert_str_value("id", group.id)
      .insert_str_value("name", group.name)
      .insert_bool_value("visible", group.visible)
      .build()
  }
}

#[derive(Debug, Clone, Default)]
pub struct TestGroupSetting {
  pub id: String,
  pub field_id: String,
  pub field_type: i64,
  pub groups: Vec<TestGroup>,
  pub content: String,
}

const GROUP_ID: &str = "id";
pub const GROUPS: &str = "groups";
pub const CONTENT: &str = "content";

impl TryFrom<&GroupSettingMap> for TestGroupSetting {
  type Error = anyhow::Error;

  fn try_from(value: &GroupSettingMap) -> Result<Self, Self::Error> {
    Self::try_from(value.clone())
  }
}

impl TryFrom<GroupSettingMap> for TestGroupSetting {
  type Error = anyhow::Error;

  fn try_from(value: GroupSettingMap) -> Result<Self, Self::Error> {
    match (
      value.get_str_value(GROUP_ID),
      value.get_str_value(FIELD_ID),
      value.get_i64_value(FIELD_TYPE),
    ) {
      (Some(id), Some(field_id), Some(field_type)) => {
        let content = value.get_str_value(CONTENT).unwrap_or_default();
        let groups = value.try_get_array(GROUPS);
        Ok(Self {
          id,
          field_id,
          field_type,
          groups,
          content,
        })
      },
      _ => {
        bail!("Invalid group setting data")
      },
    }
  }
}

impl From<TestGroupSetting> for GroupSettingMap {
  fn from(data: TestGroupSetting) -> Self {
    GroupSettingBuilder::new()
      .insert_str_value(GROUP_ID, data.id)
      .insert_str_value(FIELD_ID, data.field_id)
      .insert_i64_value(FIELD_TYPE, data.field_type)
      .insert_str_value(CONTENT, data.content)
      .insert_maps(GROUPS, data.groups)
      .build()
  }
}

#[derive(Debug, Clone)]
pub struct TestSort {
  pub id: String,
  pub field_id: String,
  pub field_type: i64,
  pub condition: SortCondition,
}

const SORT_ID: &str = "id";
const SORT_CONDITION: &str = "condition";

impl TryFrom<SortMap> for TestSort {
  type Error = anyhow::Error;

  fn try_from(value: SortMap) -> Result<Self, Self::Error> {
    match (
      value.get_str_value(SORT_ID),
      value.get_str_value(FIELD_ID),
      value.get_i64_value(FIELD_TYPE),
    ) {
      (Some(id), Some(field_id), Some(field_type)) => {
        let condition = value
          .get_i64_value(SORT_CONDITION)
          .map(|value| SortCondition::try_from(value).unwrap())
          .unwrap_or_default();

        Ok(Self {
          id,
          field_id,
          field_type,
          condition,
        })
      },
      _ => {
        bail!("Invalid group setting data")
      },
    }
  }
}

impl From<TestSort> for SortMap {
  fn from(data: TestSort) -> Self {
    SortMapBuilder::new()
      .insert_str_value(SORT_ID, data.id)
      .insert_str_value(FIELD_ID, data.field_id)
      .insert_i64_value(FIELD_TYPE, data.field_type)
      .insert_i64_value(SORT_CONDITION, data.condition.value())
      .build()
  }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum SortCondition {
  Ascending = 0,
  Descending = 1,
}

impl SortCondition {
  pub fn value(&self) -> i64 {
    *self as i64
  }
}

impl Default for SortCondition {
  fn default() -> Self {
    Self::Ascending
  }
}

impl TryFrom<i64> for SortCondition {
  type Error = anyhow::Error;

  fn try_from(value: i64) -> std::result::Result<Self, Self::Error> {
    match value {
      0 => Ok(SortCondition::Ascending),
      1 => Ok(SortCondition::Descending),
      _ => bail!("Unknown field type {}", value),
    }
  }
}

#[derive(Debug, Clone, Default)]
pub struct TestCheckboxTypeOption {
  pub is_selected: bool,
}

impl From<TypeOptionData> for TestCheckboxTypeOption {
  fn from(data: TypeOptionData) -> Self {
    let is_selected = data.get_bool_value("is_selected").unwrap_or(false);
    TestCheckboxTypeOption { is_selected }
  }
}

impl From<TestCheckboxTypeOption> for TypeOptionData {
  fn from(data: TestCheckboxTypeOption) -> Self {
    TypeOptionDataBuilder::new()
      .insert_bool_value("is_selected", data.is_selected)
      .build()
  }
}

#[derive(Clone, Debug)]
pub struct TestDateTypeOption {
  pub date_format: TestDateFormat,
  pub time_format: TestTimeFormat,
  pub include_time: bool,
}

impl From<TypeOptionData> for TestDateTypeOption {
  fn from(data: TypeOptionData) -> Self {
    let include_time = data.get_bool_value("include_time").unwrap_or(false);
    let date_format = data
      .get_i64_value("data_format")
      .map(TestDateFormat::from)
      .unwrap();
    let time_format = data
      .get_i64_value("time_format")
      .map(TestTimeFormat::from)
      .unwrap();
    Self {
      date_format,
      time_format,
      include_time,
    }
  }
}

impl From<TestDateTypeOption> for TypeOptionData {
  fn from(data: TestDateTypeOption) -> Self {
    TypeOptionDataBuilder::new()
      .insert_i64_value("data_format", data.date_format.value())
      .insert_i64_value("time_format", data.time_format.value())
      .insert_bool_value("include_time", data.include_time)
      .build()
  }
}

#[allow(clippy::upper_case_acronyms)]
#[derive(Clone, Debug, Copy, Eq, PartialEq, Default)]
pub enum TestDateFormat {
  Local = 0,
  US = 1,
  ISO = 2,
  #[default]
  Friendly = 3,
}

impl std::convert::From<i64> for TestDateFormat {
  fn from(value: i64) -> Self {
    match value {
      0 => TestDateFormat::Local,
      1 => TestDateFormat::US,
      2 => TestDateFormat::ISO,
      3 => TestDateFormat::Friendly,
      _ => {
        tracing::error!("🔴Unsupported date format, fallback to friendly");
        TestDateFormat::Friendly
      },
    }
  }
}

impl TestDateFormat {
  pub fn value(&self) -> i64 {
    *self as i64
  }
  // https://docs.rs/chrono/0.4.19/chrono/format/strftime/index.html
  pub fn format_str(&self) -> &'static str {
    match self {
      TestDateFormat::Local => "%m/%d/%Y",
      TestDateFormat::US => "%Y/%m/%d",
      TestDateFormat::ISO => "%Y-%m-%d",
      TestDateFormat::Friendly => "%b %d,%Y",
    }
  }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum TestTimeFormat {
  TwelveHour = 0,
  TwentyFourHour = 1,
}

impl std::convert::From<i64> for TestTimeFormat {
  fn from(value: i64) -> Self {
    match value {
      0 => TestTimeFormat::TwelveHour,
      1 => TestTimeFormat::TwentyFourHour,
      _ => {
        tracing::error!("🔴 Unsupported time format, fallback to TwentyFourHour");
        TestTimeFormat::TwentyFourHour
      },
    }
  }
}

impl TestTimeFormat {
  pub fn value(&self) -> i64 {
    *self as i64
  }

  // https://docs.rs/chrono/0.4.19/chrono/format/strftime/index.html
  pub fn format_str(&self) -> &'static str {
    match self {
      TestTimeFormat::TwelveHour => "%I:%M %p",
      TestTimeFormat::TwentyFourHour => "%R",
    }
  }
}

pub struct TestTextCell(pub String);

impl From<TestTextCell> for Cell {
  fn from(text_cell: TestTextCell) -> Self {
    let mut cell = Self::new();
    cell.insert("data".to_string(), Any::String(Arc::from(text_cell.0)));
    cell
  }
}

impl From<Cell> for TestTextCell {
  fn from(cell: Cell) -> Self {
    let data = cell.get_str_value("data").unwrap();
    Self(data)
  }
}

impl From<&str> for TestTextCell {
  fn from(s: &str) -> Self {
    Self(s.to_string())
  }
}
pub struct TestNumberCell(pub i64);

impl From<TestNumberCell> for Cell {
  fn from(text_cell: TestNumberCell) -> Self {
    let mut cell = Self::new();
    cell.insert_i64_value("data", text_cell.0);
    cell
  }
}

impl From<&Cell> for TestNumberCell {
  fn from(cell: &Cell) -> Self {
    let data = cell.get_i64_value("data").unwrap();
    Self(data)
  }
}

#[derive(Debug, Clone)]
pub struct TestCalendarLayoutSetting {
  pub layout_ty: TestCalendarLayout,
  pub first_day_of_week: i32,
  pub show_weekends: bool,
  pub show_week_numbers: bool,
  pub field_id: String,
}

impl From<LayoutSetting> for TestCalendarLayoutSetting {
  fn from(setting: LayoutSetting) -> Self {
    let layout_ty = setting
      .get_i64_value("layout_ty")
      .map(TestCalendarLayout::from)
      .unwrap_or_default();
    let first_day_of_week = setting
      .get_i64_value("first_day_of_week")
      .unwrap_or(DEFAULT_FIRST_DAY_OF_WEEK as i64) as i32;
    let show_weekends = setting.get_bool_value("show_weekends").unwrap_or_default();
    let show_week_numbers = setting
      .get_bool_value("show_week_numbers")
      .unwrap_or_default();
    let field_id = setting.get_str_value("field_id").unwrap_or_default();
    Self {
      layout_ty,
      first_day_of_week,
      show_weekends,
      show_week_numbers,
      field_id,
    }
  }
}

impl From<TestCalendarLayoutSetting> for LayoutSetting {
  fn from(setting: TestCalendarLayoutSetting) -> Self {
    LayoutSettingBuilder::new()
      .insert_i64_value("layout_ty", setting.layout_ty.value())
      .insert_i64_value("first_day_of_week", setting.first_day_of_week as i64)
      .insert_bool_value("show_week_numbers", setting.show_week_numbers)
      .insert_bool_value("show_weekends", setting.show_weekends)
      .insert_str_value("field_id", setting.field_id)
      .build()
  }
}

impl TestCalendarLayoutSetting {
  pub fn new(field_id: String) -> Self {
    TestCalendarLayoutSetting {
      layout_ty: TestCalendarLayout::default(),
      first_day_of_week: DEFAULT_FIRST_DAY_OF_WEEK,
      show_weekends: DEFAULT_SHOW_WEEKENDS,
      show_week_numbers: DEFAULT_SHOW_WEEK_NUMBERS,
      field_id,
    }
  }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Default)]
#[repr(u8)]
pub enum TestCalendarLayout {
  #[default]
  Month = 0,
  Week = 1,
  Day = 2,
}

impl From<i64> for TestCalendarLayout {
  fn from(value: i64) -> Self {
    match value {
      0 => TestCalendarLayout::Month,
      1 => TestCalendarLayout::Week,
      2 => TestCalendarLayout::Day,
      _ => TestCalendarLayout::Month,
    }
  }
}

impl TestCalendarLayout {
  pub fn value(&self) -> i64 {
    *self as i64
  }
}

pub const DEFAULT_FIRST_DAY_OF_WEEK: i32 = 0;
pub const DEFAULT_SHOW_WEEKENDS: bool = true;
pub const DEFAULT_SHOW_WEEK_NUMBERS: bool = true;

#[derive(Debug, Clone, Eq, PartialEq)]
#[repr(u8)]
#[derive(Default)]
pub enum TestFieldType {
  #[default]
  RichText = 0,
  Number = 1,
  DateTime = 2,
  SingleSelect = 3,
  MultiSelect = 4,
  Checkbox = 5,
  URL = 6,
  Checklist = 7,
}

impl From<TestFieldType> for i64 {
  fn from(ty: TestFieldType) -> Self {
    ty as i64
  }
}

impl std::convert::From<i64> for TestFieldType {
  fn from(ty: i64) -> Self {
    match ty {
      0 => TestFieldType::RichText,
      1 => TestFieldType::Number,
      2 => TestFieldType::DateTime,
      3 => TestFieldType::SingleSelect,
      4 => TestFieldType::MultiSelect,
      5 => TestFieldType::Checkbox,
      6 => TestFieldType::URL,
      7 => TestFieldType::Checklist,
      _ => {
        tracing::error!("🔴Can't parser FieldType from value: {}", ty);
        TestFieldType::RichText
      },
    }
  }
}

#[derive(Debug, Clone)]
pub struct TestFieldSetting {
  pub width: i32,
  pub visibility: u8,
}

impl Default for TestFieldSetting {
  fn default() -> Self {
    Self::new()
  }
}

impl TestFieldSetting {
  pub fn new() -> Self {
    Self {
      width: 0,
      visibility: 0,
    }
  }
}

const VISIBILITY: &str = "visibility";

impl From<FieldSettingsMap> for TestFieldSetting {
  fn from(value: FieldSettingsMap) -> Self {
    TestFieldSetting {
      width: value.get_i64_value("width").unwrap_or(0) as i32,
      visibility: value.get_i64_value(VISIBILITY).unwrap_or(0) as u8,
    }
  }
}

impl From<TestFieldSetting> for AnyMap {
  fn from(data: TestFieldSetting) -> Self {
    FieldSettingsMapBuilder::new()
      .insert_i64_value("width", data.width as i64)
      .insert_i64_value(VISIBILITY, data.visibility as i64)
      .build()
  }
}

pub fn make_rocks_db() -> Arc<CollabKVDB> {
  let tempdir = TempDir::new().unwrap();
  let path = tempdir.into_path();
  Arc::new(CollabKVDB::open(path).unwrap())
}

pub fn setup_log() {
  static START: Once = Once::new();
  START.call_once(|| {
    let level = "info";
    let mut filters = vec![];
    filters.push(format!("collab_persistence={}", level));
    filters.push(format!("collab={}", level));
    filters.push(format!("collab_database={}", level));
    std::env::set_var("RUST_LOG", filters.join(","));
    let subscriber = Subscriber::builder()
      .with_env_filter(EnvFilter::from_default_env())
      .with_ansi(true)
      .finish();
    subscriber.try_init().unwrap();
  });
}

pub fn unzip_history_database_db(folder_name: &str) -> std::io::Result<(Cleaner, PathBuf)> {
  // Open the zip file
  let zip_file_path = format!("./tests/history_database/{}.zip", folder_name);
  let reader = File::open(zip_file_path)?;
  let output_folder_path = format!("./tests/history_document/unit_test_{}", nanoid!(6));

  // Create a ZipArchive from the file
  let mut archive = ZipArchive::new(reader)?;

  // Iterate through each file in the zip
  for i in 0..archive.len() {
    let mut file = archive.by_index(i)?;
    let outpath = Path::new(&output_folder_path).join(file.mangled_name());

    if file.name().ends_with('/') {
      // Create directory
      create_dir_all(&outpath)?;
    } else {
      // Write file
      if let Some(p) = outpath.parent() {
        if !p.exists() {
          create_dir_all(p)?;
        }
      }
      let mut outfile = File::create(&outpath)?;
      copy(&mut file, &mut outfile)?;
    }
  }
  let path = format!("{}/{}", output_folder_path, folder_name);
  Ok((
    Cleaner::new(PathBuf::from(output_folder_path)),
    PathBuf::from(path),
  ))
}

pub struct Cleaner(PathBuf);

impl Cleaner {
  pub fn new(dir: PathBuf) -> Self {
    Cleaner(dir)
  }

  fn cleanup(dir: &PathBuf) {
    let _ = std::fs::remove_dir_all(dir);
  }
}

impl Drop for Cleaner {
  fn drop(&mut self) {
    Self::cleanup(&self.0)
  }
}
