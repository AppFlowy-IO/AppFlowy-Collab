use collab::plugin_impl::disk::CollabDiskPlugin;
use collab::preclude::CollabBuilder;
use collab_database::database::{Database, DatabaseContext};
use collab_database::fields::{Field, FieldType};
use collab_database::rows::Row;
use collab_database::views::{CreateViewParams, Layout, View};
use collab_persistence::CollabKV;
use std::ops::{Deref, DerefMut};
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::TempDir;

pub struct DatabaseTest {
  database: Database,

  #[allow(dead_code)]
  cleaner: Cleaner,
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
  let db = Arc::new(CollabKV::open(path.clone()).unwrap());
  let disk_plugin = CollabDiskPlugin::new(uid, db).unwrap();
  let cleaner = Cleaner::new(path);

  let collab = CollabBuilder::new(1, database_id)
    .with_plugin(disk_plugin)
    .build();
  collab.initial();
  let context = DatabaseContext {};
  let database = Database::create(database_id, collab, context).unwrap();
  DatabaseTest { database, cleaner }
}

pub fn create_database_with_default_data(uid: i64, database_id: &str) -> DatabaseTest {
  let row_1 = Row {
    id: "r1".to_string(),
    cells: Default::default(),
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
  database_test.insert_row(row_1);
  database_test.insert_row(row_2);
  database_test.insert_row(row_3);

  let field_1 = Field::new(
    "f1".to_string(),
    "text field".to_string(),
    FieldType::RichText,
    true,
  );

  let field_2 = Field::new(
    "f2".to_string(),
    "single select field".to_string(),
    FieldType::SingleSelect,
    true,
  );

  let field_3 = Field::new(
    "f3".to_string(),
    "checkbox field".to_string(),
    FieldType::Checkbox,
    true,
  );

  database_test.insert_field(field_1);
  database_test.insert_field(field_2);
  database_test.insert_field(field_3);

  database_test
}

pub fn create_database_grid_view(uid: i64, database_id: &str, view_id: &str) -> DatabaseTest {
  let database_test = create_database_with_default_data(uid, database_id);
  let params = CreateViewParams {
    id: view_id.to_string(),
    name: "my first grid".to_string(),
    layout: Layout::Grid,
    ..Default::default()
  };
  database_test.create_view(params);
  database_test
}

struct Cleaner(PathBuf);

impl Cleaner {
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

pub fn make_grid_view(id: &str, name: &str) -> View {
  View {
    id: id.to_string(),
    database_id: "".to_string(),
    name: name.to_string(),
    layout: Layout::Grid,
    layout_settings: Default::default(),
    filters: vec![],
    groups: vec![],
    sorts: vec![],
    row_orders: vec![],
    field_orders: vec![],
    created_at: 0,
    modified_at: 0,
  }
}
