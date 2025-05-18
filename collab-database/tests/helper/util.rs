#![allow(clippy::upper_case_acronyms)]

use std::fs::{File, create_dir_all};
use std::io::copy;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Once};

use anyhow::bail;
use collab::preclude::encoding::serde::from_any;
use collab::preclude::{Any, any};
use collab::util::AnyMapExt;
use collab_database::fields::{TypeOptionData, TypeOptionDataBuilder};
use collab_database::rows::Cell;
use collab_database::views::{
  FieldSettingsMap, FilterMap, FilterMapBuilder, GroupMap, GroupMapBuilder, GroupSettingBuilder,
  GroupSettingMap, LayoutSetting, LayoutSettingBuilder, SortMap, SortMapBuilder,
};
use collab_plugins::CollabKVDB;
use nanoid::nanoid;
use serde::Deserialize;
use serde_repr::{Deserialize_repr, Serialize_repr};
use tempfile::TempDir;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt::Subscriber;
use tracing_subscriber::util::SubscriberInitExt;
use zip::ZipArchive;

#[derive(Debug, Clone, Deserialize)]
pub struct TestFilter {
  pub id: String,
  pub field_id: String,
  #[serde(default, rename = "ty")]
  pub field_type: TestFieldType,
  #[serde(default)]
  pub condition: i64,
  #[serde(default)]
  pub content: String,
}

const FILTER_ID: &str = "id";
pub const FIELD_ID: &str = "field_id";
pub const FIELD_TYPE: &str = "ty";
pub const FILTER_CONDITION: &str = "condition";
pub const FILTER_CONTENT: &str = "content";

impl From<TestFilter> for FilterMap {
  fn from(data: TestFilter) -> Self {
    FilterMapBuilder::from([
      (FILTER_ID.into(), data.id.into()),
      (FIELD_ID.into(), data.field_id.into()),
      (FILTER_CONTENT.into(), data.content.into()),
      (FIELD_TYPE.into(), i64::from(data.field_type).into()),
      (FILTER_CONDITION.into(), data.condition.into()),
    ])
  }
}

impl TryFrom<FilterMap> for TestFilter {
  type Error = anyhow::Error;

  fn try_from(filter: FilterMap) -> Result<Self, Self::Error> {
    let any = Any::from(filter);
    Ok(from_any(&any)?)
  }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct TestGroup {
  pub id: String,
  #[serde(default)]
  pub name: String,
  #[serde(default)]
  pub visible: bool,
}

impl From<GroupMap> for TestGroup {
  fn from(value: GroupMap) -> Self {
    let any = Any::from(value);
    from_any(&any).unwrap()
  }
}

impl From<TestGroup> for GroupMap {
  fn from(group: TestGroup) -> Self {
    GroupMapBuilder::from([
      ("id".into(), group.id.into()),
      ("name".into(), group.name.into()),
      ("visible".into(), group.visible.into()),
    ])
  }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct TestGroupSetting {
  pub id: String,
  pub field_id: String,
  #[serde(rename = "ty")]
  pub field_type: i64,
  #[serde(default)]
  pub groups: Vec<TestGroup>,
  #[serde(default)]
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
    let any = Any::from(value);
    Ok(from_any(&any)?)
  }
}

impl From<TestGroupSetting> for GroupSettingMap {
  fn from(data: TestGroupSetting) -> Self {
    let groups: Vec<Any> = data
      .groups
      .into_iter()
      .map(|group| GroupMap::from(group).into())
      .collect();
    GroupSettingBuilder::from([
      (GROUP_ID.into(), data.id.into()),
      (FIELD_ID.into(), data.field_id.into()),
      (FIELD_TYPE.into(), data.field_type.into()),
      (CONTENT.into(), data.content.into()),
      (GROUPS.into(), groups.into()),
    ])
  }
}

#[derive(Debug, Clone, Deserialize)]
pub struct TestSort {
  pub id: String,
  pub field_id: String,
  #[serde(rename = "ty")]
  pub field_type: i64,
  #[serde(default)]
  pub condition: SortCondition,
}

const SORT_ID: &str = "id";
const SORT_CONDITION: &str = "condition";

impl TryFrom<SortMap> for TestSort {
  type Error = anyhow::Error;

  fn try_from(value: SortMap) -> Result<Self, Self::Error> {
    let any = Any::from(value);
    Ok(from_any(&any)?)
  }
}

impl From<TestSort> for SortMap {
  fn from(data: TestSort) -> Self {
    SortMapBuilder::from([
      (SORT_ID.into(), data.id.into()),
      (FIELD_ID.into(), data.field_id.into()),
      (FIELD_TYPE.into(), data.field_type.into()),
      (SORT_CONDITION.into(), data.condition.value().into()),
    ])
  }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Deserialize_repr)]
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

#[derive(Debug, Clone, Default, Deserialize)]
pub struct TestCheckboxTypeOption {
  #[serde(default)]
  pub is_selected: bool,
}

impl From<TypeOptionData> for TestCheckboxTypeOption {
  fn from(data: TypeOptionData) -> Self {
    let any = Any::from(data);
    from_any(&any).unwrap()
  }
}

impl From<TestCheckboxTypeOption> for TypeOptionData {
  fn from(data: TestCheckboxTypeOption) -> Self {
    TypeOptionDataBuilder::from([("is_selected".into(), data.is_selected.into())])
  }
}

#[derive(Clone, Debug, Deserialize)]
pub struct TestDateTypeOption {
  #[serde(alias = "data_format")] // it's probably a typo, but we're in that universe somehow
  pub date_format: TestDateFormat,
  pub time_format: TestTimeFormat,
  #[serde(default)]
  pub include_time: bool,
}

impl From<TypeOptionData> for TestDateTypeOption {
  fn from(data: TypeOptionData) -> Self {
    let any = Any::from(data);
    from_any(&any).unwrap()
  }
}

impl From<TestDateTypeOption> for TypeOptionData {
  fn from(data: TestDateTypeOption) -> Self {
    TypeOptionDataBuilder::from([
      ("data_format".into(), data.date_format.value().into()),
      ("time_format".into(), data.time_format.value().into()),
      ("include_time".into(), data.include_time.into()),
    ])
  }
}

#[allow(clippy::upper_case_acronyms)]
#[repr(i64)]
#[derive(Clone, Debug, Copy, Eq, PartialEq, Default, Deserialize_repr)]
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

#[repr(i64)]
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, Deserialize_repr)]
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
    match cell.get("data") {
      Some(Any::String(data)) => Self(data.to_string()),
      _ => unreachable!(),
    }
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
    Self::from([("data".into(), text_cell.0.into())])
  }
}

impl From<&Cell> for TestNumberCell {
  fn from(cell: &Cell) -> Self {
    Self(cell.get_as("data").unwrap())
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
      .get_as::<i64>("layout_ty")
      .map(TestCalendarLayout::from)
      .unwrap_or_default();
    let first_day_of_week = setting
      .get_as("first_day_of_week")
      .unwrap_or(DEFAULT_FIRST_DAY_OF_WEEK as i64) as i32;
    let show_weekends: bool = setting.get_as("show_weekends").unwrap_or_default();
    let show_week_numbers: bool = setting.get_as("show_week_numbers").unwrap_or_default();
    let field_id: String = setting.get_as("field_id").unwrap_or_default();
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
    LayoutSettingBuilder::from([
      ("layout_ty".into(), setting.layout_ty.value().into()),
      (
        "first_day_of_week".into(),
        (setting.first_day_of_week as i64).into(),
      ),
      ("show_week_numbers".into(), setting.show_week_numbers.into()),
      ("show_weekends".into(), setting.show_weekends.into()),
      ("field_id".into(), setting.field_id.into()),
    ])
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

#[derive(Debug, Clone, Eq, PartialEq, Deserialize_repr, Serialize_repr)]
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

#[derive(Debug, Clone, Deserialize)]
pub struct TestFieldSetting {
  #[serde(default)]
  pub width: i32,
  #[serde(default)]
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
    from_any(&Any::from(value)).unwrap()
  }
}

impl From<TestFieldSetting> for FieldSettingsMap {
  fn from(value: TestFieldSetting) -> Self {
    FieldSettingsMap::from([
      ("width".into(), value.width.into()),
      (VISIBILITY.into(), (value.visibility as i64).into()),
    ])
  }
}

impl From<TestFieldSetting> for Any {
  fn from(data: TestFieldSetting) -> Self {
    any!({
      "width": (data.width as i64),
      "visibility": (data.visibility as i64),
    })
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
    let level = "trace";
    let mut filters = vec![];
    filters.push(format!("collab_persistence={}", level));
    filters.push(format!("collab={}", level));
    filters.push(format!("collab_database={}", level));
    unsafe {
      std::env::set_var("RUST_LOG", filters.join(","));
    }
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
