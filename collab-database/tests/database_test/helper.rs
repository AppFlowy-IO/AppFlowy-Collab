use collab::core::any_map::AnyMapExtension;
use collab::plugin_impl::disk::CollabDiskPlugin;
use collab::plugin_impl::snapshot::CollabSnapshotPlugin;
use collab::preclude::CollabBuilder;
use collab_database::database::{Database, DatabaseContext};
use collab_database::fields::Field;
use collab_database::rows::{CellsBuilder, Row};
use collab_database::views::{
  CreateViewParams, DatabaseLayout, FilterMap, FilterMapBuilder, GroupMap, GroupMapBuilder,
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
  let collab = CollabBuilder::new(uid, database_id).build();
  collab.initial();
  let context = DatabaseContext { collab };
  let database = Database::create(database_id, context).unwrap();
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
  let context = DatabaseContext { collab };
  let database = Database::create(database_id, context).unwrap();
  (
    db,
    DatabaseTest {
      database,
      cleaner: None,
    },
  )
}

pub fn create_database_from_db(uid: i64, database_id: &str, db: Arc<CollabKV>) -> DatabaseTest {
  let disk_plugin = CollabDiskPlugin::new(uid, db.clone()).unwrap();
  let snapshot_plugin = CollabSnapshotPlugin::new(uid, db, 5).unwrap();
  let collab = CollabBuilder::new(uid, database_id)
    .with_plugin(disk_plugin)
    .with_plugin(snapshot_plugin)
    .build();
  collab.initial();
  let context = DatabaseContext { collab };
  let database = Database::create(database_id, context).unwrap();
  DatabaseTest {
    database,
    cleaner: None,
  }
}

pub fn create_database_with_default_data(uid: i64, database_id: &str) -> DatabaseTest {
  let row_1 = Row {
    id: "r1".to_string(),
    cells: CellsBuilder::new().insert_text_cell("f1", "123").build(),
    height: 0,
    visibility: true,
    created_at: 1,
  };
  let row_2 = Row {
    id: "r2".to_string(),
    cells: Default::default(),
    height: 0,
    visibility: true,
    created_at: 2,
  };
  let row_3 = Row {
    id: "r3".to_string(),
    cells: Default::default(),
    height: 0,
    visibility: true,
    created_at: 3,
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

pub fn create_database_grid_view(uid: i64, database_id: &str, view_id: &str) -> DatabaseTest {
  let database_test = create_database_with_default_data(uid, database_id);
  let params = CreateViewParams {
    view_id: view_id.to_string(),
    name: "my first grid".to_string(),
    layout: DatabaseLayout::Grid,
    ..Default::default()
  };
  database_test.create_view(params);
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
