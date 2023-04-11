use anyhow::bail;
use collab::core::any_map::AnyMapExtension;
use collab::plugin_impl::disk::CollabDiskPlugin;
use collab::plugin_impl::snapshot::CollabSnapshotPlugin;
use collab::preclude::{lib0Any, CollabBuilder};
use collab_database::block::{Blocks, CreateRowParams};
use collab_database::database::{Database, DatabaseContext};
use collab_database::fields::{Field, TypeOptionData, TypeOptionDataBuilder};
use collab_database::rows::{Cell, CellsBuilder};
use collab_database::views::{
  CreateDatabaseParams, FilterMap, FilterMapBuilder, GroupMap, GroupMapBuilder,
  GroupSettingBuilder, GroupSettingMap, LayoutSetting, LayoutSettingBuilder, SortMap,
  SortMapBuilder,
};
use collab_persistence::CollabKV;
use std::ops::{Deref, DerefMut};
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::TempDir;

pub struct DatabaseTest {
  database: Database,

  #[allow(dead_code)]
  cleaner: Option<Cleaner>,
}

unsafe impl Send for DatabaseTest {}

unsafe impl Sync for DatabaseTest {}

impl Deref for DatabaseTest {
  type Target = Database;

  fn deref(&self) -> &Self::Target {
    &self.database
  }
}

impl DerefMut for DatabaseTest {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.database
  }
}

pub fn create_database(uid: i64, database_id: &str) -> DatabaseTest {
  let tempdir = TempDir::new().unwrap();
  let path = tempdir.into_path();
  let db = Arc::new(CollabKV::open(path).unwrap());
  let collab = CollabBuilder::new(uid, database_id).build();
  collab.initial();
  let blocks = Blocks::new(uid, db.clone());
  let context = DatabaseContext { collab, blocks };
  let params = CreateDatabaseParams {
    database_id: database_id.to_string(),
    view_id: "v1".to_string(),
    ..Default::default()
  };
  let database = Database::create_with_view(database_id, params, context).unwrap();
  DatabaseTest {
    database,
    cleaner: None,
  }
}

pub fn create_database_with_db(uid: i64, database_id: &str) -> (Arc<CollabKV>, DatabaseTest) {
  let tempdir = TempDir::new().unwrap();
  let path = tempdir.into_path();
  let db = Arc::new(CollabKV::open(path).unwrap());
  let disk_plugin = CollabDiskPlugin::new(uid, db.clone()).unwrap();
  let snapshot_plugin = CollabSnapshotPlugin::new(uid, db.clone(), 5).unwrap();

  let collab = CollabBuilder::new(1, database_id)
    .with_plugin(disk_plugin)
    .with_plugin(snapshot_plugin)
    .build();
  collab.initial();
  let blocks = Blocks::new(uid, db.clone());
  let context = DatabaseContext { collab, blocks };
  let params = CreateDatabaseParams {
    view_id: "v1".to_string(),
    name: "my first grid".to_string(),
    ..Default::default()
  };
  let database = Database::create_with_view(database_id, params, context).unwrap();
  (
    db,
    DatabaseTest {
      database,
      cleaner: None,
    },
  )
}

pub fn restore_database_from_db(uid: i64, database_id: &str, db: Arc<CollabKV>) -> DatabaseTest {
  let disk_plugin = CollabDiskPlugin::new(uid, db.clone()).unwrap();
  let blocks = Blocks::new(uid, db.clone());
  let snapshot_plugin = CollabSnapshotPlugin::new(uid, db, 5).unwrap();
  let collab = CollabBuilder::new(uid, database_id)
    .with_plugin(disk_plugin)
    .with_plugin(snapshot_plugin)
    .build();
  collab.initial();
  let context = DatabaseContext { collab, blocks };
  let database = Database::get_or_create(database_id, context).unwrap();
  DatabaseTest {
    database,
    cleaner: None,
  }
}

/// Create a database with default data
/// It will create a default view with id 'v1'
pub fn create_database_with_default_data(uid: i64, database_id: &str) -> DatabaseTest {
  let row_1 = CreateRowParams {
    id: 1.into(),
    cells: CellsBuilder::new()
      .insert_cell("f1", TestTextCell::from("1f1cell"))
      .insert_cell("f2", TestTextCell::from("1f2cell"))
      .insert_cell("f3", TestTextCell::from("1f3cell"))
      .build(),
    height: 0,
    visibility: true,
    prev_row_id: None,
  };
  let row_2 = CreateRowParams {
    id: 2.into(),
    cells: CellsBuilder::new()
      .insert_cell("f1", TestTextCell::from("2f1cell"))
      .insert_cell("f2", TestTextCell::from("2f2cell"))
      .build(),
    height: 0,
    visibility: true,
    prev_row_id: None,
  };
  let row_3 = CreateRowParams {
    id: 3.into(),
    cells: CellsBuilder::new()
      .insert_cell("f1", TestTextCell::from("3f1cell"))
      .insert_cell("f3", TestTextCell::from("3f3cell"))
      .build(),
    height: 0,
    visibility: true,
    prev_row_id: None,
  };

  let database_test = create_database(uid, database_id);
  database_test.push_row(row_1);
  database_test.push_row(row_2);
  database_test.push_row(row_3);

  let field_1 = Field::new("f1".to_string(), "text field".to_string(), 0, true);
  let field_2 = Field::new("f2".to_string(), "single select field".to_string(), 2, true);
  let field_3 = Field::new("f3".to_string(), "checkbox field".to_string(), 1, true);

  database_test.insert_field(field_1);
  database_test.insert_field(field_2);
  database_test.insert_field(field_3);

  database_test
}

pub fn create_database_grid_view(uid: i64, database_id: &str) -> DatabaseTest {
  let database_test = create_database_with_default_data(uid, database_id);
  database_test
}

struct Cleaner(PathBuf);

impl Cleaner {
  #[allow(dead_code)]
  fn new(dir: PathBuf) -> Self {
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

#[derive(Debug, Clone)]
pub struct TestFilter {
  pub id: String,
  pub field_id: String,
  pub field_type: i64,
  pub condition: i64,
  pub content: String,
}

const FILTER_ID: &str = "id";
const FIELD_ID: &str = "field_id";
const FIELD_TYPE: &str = "ty";
const FILTER_CONDITION: &str = "condition";
const FILTER_CONTENT: &str = "content";

impl From<TestFilter> for FilterMap {
  fn from(data: TestFilter) -> Self {
    FilterMapBuilder::new()
      .insert_str_value(FILTER_ID, data.id)
      .insert_str_value(FIELD_ID, data.field_id)
      .insert_str_value(FILTER_CONTENT, data.content)
      .insert_i64_value(FIELD_TYPE, data.field_type)
      .insert_i64_value(FILTER_CONDITION, data.condition)
      .build()
  }
}

impl From<FilterMap> for TestFilter {
  fn from(filter: FilterMap) -> Self {
    let id = filter.get_str_value(FILTER_ID).unwrap();
    let field_id = filter.get_str_value(FIELD_ID).unwrap();
    let condition = filter.get_i64_value(FILTER_CONDITION).unwrap_or(0);
    let content = filter.get_str_value(FILTER_CONTENT).unwrap_or_default();
    let field_type = filter.get_i64_value(FIELD_TYPE).unwrap_or_default();
    TestFilter {
      id,
      field_id,
      field_type,
      condition,
      content,
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
#[derive(Clone, Debug, Copy, Eq, PartialEq)]
pub enum TestDateFormat {
  Local = 0,
  US = 1,
  ISO = 2,
  Friendly = 3,
}

impl std::default::Default for TestDateFormat {
  fn default() -> Self {
    TestDateFormat::Friendly
  }
}

impl std::convert::From<i64> for TestDateFormat {
  fn from(value: i64) -> Self {
    match value {
      0 => TestDateFormat::Local,
      1 => TestDateFormat::US,
      2 => TestDateFormat::ISO,
      3 => TestDateFormat::Friendly,
      _ => {
        tracing::error!("Unsupported date format, fallback to friendly");
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
        tracing::error!("Unsupported time format, fallback to TwentyFourHour");
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
    cell.insert(
      "data".to_string(),
      lib0Any::String(text_cell.0.into_boxed_str()),
    );
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

#[derive(Debug, Clone)]
pub struct TestCalendarLayoutSetting {
  pub layout_ty: CalendarLayout,
  pub first_day_of_week: i32,
  pub show_weekends: bool,
  pub show_week_numbers: bool,
  pub field_id: String,
}

impl From<LayoutSetting> for TestCalendarLayoutSetting {
  fn from(setting: LayoutSetting) -> Self {
    let layout_ty = setting
      .get_i64_value("layout_ty")
      .map(CalendarLayout::from)
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
      layout_ty: CalendarLayout::default(),
      first_day_of_week: DEFAULT_FIRST_DAY_OF_WEEK,
      show_weekends: DEFAULT_SHOW_WEEKENDS,
      show_week_numbers: DEFAULT_SHOW_WEEK_NUMBERS,
      field_id,
    }
  }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Default)]
#[repr(u8)]
pub enum CalendarLayout {
  #[default]
  MonthLayout = 0,
  WeekLayout = 1,
  DayLayout = 2,
}

impl From<i64> for CalendarLayout {
  fn from(value: i64) -> Self {
    match value {
      0 => CalendarLayout::MonthLayout,
      1 => CalendarLayout::WeekLayout,
      2 => CalendarLayout::DayLayout,
      _ => CalendarLayout::MonthLayout,
    }
  }
}

impl CalendarLayout {
  pub fn value(&self) -> i64 {
    *self as i64
  }
}

pub const DEFAULT_FIRST_DAY_OF_WEEK: i32 = 0;
pub const DEFAULT_SHOW_WEEKENDS: bool = true;
pub const DEFAULT_SHOW_WEEK_NUMBERS: bool = true;
