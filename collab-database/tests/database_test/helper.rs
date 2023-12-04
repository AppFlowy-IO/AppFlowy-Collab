use std::collections::HashMap;
use std::ops::{Deref, DerefMut};
use std::path::PathBuf;
use std::sync::Arc;

use collab::core::collab::CollabRawData;
use collab::preclude::CollabBuilder;
use collab_database::database::{Database, DatabaseContext};
use collab_database::fields::Field;
use collab_database::rows::{CellsBuilder, CreateRowParams};
use collab_database::user::DatabaseCollabService;
use collab_database::views::{
  CreateDatabaseParams, DatabaseLayout, FieldSettingsByFieldIdMap, FieldSettingsMap, LayoutSetting,
  LayoutSettings, OrderObjectPosition,
};
use collab_entity::CollabType;
use collab_persistence::kv::rocks_kv::RocksCollabDB;
use collab_plugins::local_storage::CollabPersistenceConfig;

use collab_database::database_observer::DatabaseNotify;
use tempfile::TempDir;

pub use crate::helper::*;
use crate::user_test::helper::TestUserDatabaseCollabBuilderImpl;

pub struct DatabaseTest {
  #[allow(dead_code)]
  collab_db: Arc<RocksCollabDB>,
  database: Database,
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

/// Create a database with a single view.
pub async fn create_database(uid: i64, database_id: &str) -> DatabaseTest {
  setup_log();
  let collab_db = make_rocks_db();
  let collab = CollabBuilder::new(uid, database_id)
    .with_device_id("1")
    .build()
    .unwrap();
  collab.lock().initialize();
  let collab_builder = Arc::new(TestUserDatabaseCollabBuilderImpl());
  let context = DatabaseContext {
    uid,
    db: Arc::downgrade(&collab_db),
    collab: Arc::new(collab),
    collab_service: collab_builder,
    notifier: Some(DatabaseNotify::default()),
  };
  let params = CreateDatabaseParams {
    database_id: database_id.to_string(),
    view_id: "v1".to_string(),
    name: "my first database view".to_string(),
    ..Default::default()
  };
  let database = Database::create_with_inline_view(params, context).unwrap();
  DatabaseTest {
    database,
    collab_db,
  }
}

pub async fn create_database_with_db(
  uid: i64,
  database_id: &str,
) -> (Arc<RocksCollabDB>, DatabaseTest) {
  setup_log();
  let collab_db = make_rocks_db();
  let collab_builder = Arc::new(TestUserDatabaseCollabBuilderImpl());
  let collab = collab_builder.build_collab_with_config(
    uid,
    database_id,
    CollabType::Database,
    Arc::downgrade(&collab_db),
    CollabRawData::default(),
    &CollabPersistenceConfig::default(),
  );
  let context = DatabaseContext {
    uid,
    db: Arc::downgrade(&collab_db),
    collab,
    collab_service: collab_builder,
    notifier: Some(DatabaseNotify::default()),
  };
  let params = CreateDatabaseParams {
    view_id: "v1".to_string(),
    name: "my first grid".to_string(),
    database_id: database_id.to_string(),
    ..Default::default()
  };
  let database = Database::create_with_inline_view(params, context).unwrap();
  (
    collab_db.clone(),
    DatabaseTest {
      database,
      collab_db,
    },
  )
}

pub fn restore_database_from_db(
  uid: i64,
  database_id: &str,
  collab_db: Arc<RocksCollabDB>,
) -> DatabaseTest {
  let collab_builder = Arc::new(TestUserDatabaseCollabBuilderImpl());
  let collab = collab_builder.build_collab_with_config(
    uid,
    database_id,
    CollabType::Database,
    Arc::downgrade(&collab_db),
    CollabRawData::default(),
    &CollabPersistenceConfig::default(),
  );
  let context = DatabaseContext {
    uid,
    db: Arc::downgrade(&collab_db),
    collab,
    collab_service: collab_builder,
    notifier: Some(DatabaseNotify::default()),
  };
  let database = Database::get_or_create(database_id, context).unwrap();
  DatabaseTest {
    database,
    collab_db,
  }
}

pub struct DatabaseTestBuilder {
  uid: i64,
  database_id: String,
  view_id: String,
  rows: Vec<CreateRowParams>,
  layout_settings: LayoutSettings,
  fields: Vec<Field>,
  layout: DatabaseLayout,
  field_settings: FieldSettingsByFieldIdMap,
}

impl DatabaseTestBuilder {
  pub fn new(uid: i64, database_id: &str) -> Self {
    Self {
      uid,
      database_id: database_id.to_string(),
      view_id: "v1".to_string(),
      rows: vec![],
      layout_settings: Default::default(),
      fields: vec![],
      layout: DatabaseLayout::Grid,
      field_settings: Default::default(),
    }
  }

  pub fn with_row(mut self, row: CreateRowParams) -> Self {
    self.rows.push(row);
    self
  }

  pub fn with_layout_setting(mut self, layout_setting: LayoutSetting) -> Self {
    self.layout_settings.insert(self.layout, layout_setting);
    self
  }

  pub fn with_field(mut self, field: Field) -> Self {
    self.fields.push(field);
    self
  }

  pub fn with_layout(mut self, layout: DatabaseLayout) -> Self {
    self.layout = layout;
    self
  }

  pub async fn build(self) -> DatabaseTest {
    let tempdir = TempDir::new().unwrap();
    let path = tempdir.into_path();
    let collab_db = Arc::new(RocksCollabDB::open_opt(path, false).unwrap());
    let collab = CollabBuilder::new(self.uid, &self.database_id)
      .with_device_id("1")
      .build()
      .unwrap();
    collab.lock().initialize();
    let collab_builder = Arc::new(TestUserDatabaseCollabBuilderImpl());
    let context = DatabaseContext {
      uid: self.uid,
      db: Arc::downgrade(&collab_db),
      collab: Arc::new(collab),
      collab_service: collab_builder,
      notifier: Some(DatabaseNotify::default()),
    };
    let params = CreateDatabaseParams {
      database_id: self.database_id.clone(),
      view_id: self.view_id,
      name: "my first database view".to_string(),
      layout: self.layout,
      layout_settings: self.layout_settings,
      filters: vec![],
      groups: vec![],
      sorts: vec![],
      field_settings: self.field_settings,
      created_rows: self.rows,
      fields: self.fields,
    };
    let database = Database::create_with_inline_view(params, context).unwrap();
    DatabaseTest {
      database,
      collab_db,
    }
  }
}

/// Create a database with default data
/// It will create a default view with id 'v1'
pub async fn create_database_with_default_data(uid: i64, database_id: &str) -> DatabaseTest {
  let row_1 = CreateRowParams {
    id: 1.into(),
    cells: CellsBuilder::new()
      .insert_cell("f1", TestTextCell::from("1f1cell"))
      .insert_cell("f2", TestTextCell::from("1f2cell"))
      .insert_cell("f3", TestTextCell::from("1f3cell"))
      .build(),
    height: 0,
    visibility: true,
    row_position: OrderObjectPosition::default(),
    timestamp: 0,
  };
  let row_2 = CreateRowParams {
    id: 2.into(),
    cells: CellsBuilder::new()
      .insert_cell("f1", TestTextCell::from("2f1cell"))
      .insert_cell("f2", TestTextCell::from("2f2cell"))
      .build(),
    height: 0,
    visibility: true,
    row_position: OrderObjectPosition::default(),
    timestamp: 0,
  };
  let row_3 = CreateRowParams {
    id: 3.into(),
    cells: CellsBuilder::new()
      .insert_cell("f1", TestTextCell::from("3f1cell"))
      .insert_cell("f3", TestTextCell::from("3f3cell"))
      .build(),
    height: 0,
    visibility: true,
    row_position: OrderObjectPosition::default(),
    timestamp: 0,
  };

  let database_test = create_database(uid, database_id).await;
  database_test.create_row(row_1).unwrap();
  database_test.create_row(row_2).unwrap();
  database_test.create_row(row_3).unwrap();

  let field_1 = Field::new("f1".to_string(), "text field".to_string(), 0, true);
  let field_2 = Field::new("f2".to_string(), "single select field".to_string(), 2, true);
  let field_3 = Field::new("f3".to_string(), "checkbox field".to_string(), 1, true);

  let field_settings_by_layout = default_field_settings_by_layout();

  database_test.create_field(
    None,
    field_1,
    &OrderObjectPosition::default(),
    field_settings_by_layout.clone(),
  );
  database_test.create_field(
    None,
    field_2,
    &OrderObjectPosition::default(),
    field_settings_by_layout.clone(),
  );
  database_test.create_field(
    None,
    field_3,
    &OrderObjectPosition::default(),
    field_settings_by_layout,
  );

  database_test.set_field_settings("v1", field_settings_for_default_database());

  database_test
}

/// Creates the default field settings for the database created by
/// create_database_with_default_data
pub fn field_settings_for_default_database() -> FieldSettingsByFieldIdMap {
  let field_settings = FieldSettingsMap::from(TestFieldSetting {
    width: 0,
    visibility: 0,
  });
  let mut field_settings_map = HashMap::new();
  field_settings_map.insert("f1".to_string(), field_settings.clone());
  field_settings_map.insert("f2".to_string(), field_settings.clone());
  field_settings_map.insert("f3".to_string(), field_settings);
  field_settings_map.into()
}

pub fn default_field_settings_by_layout() -> HashMap<DatabaseLayout, FieldSettingsMap> {
  let field_settings = FieldSettingsMap::from(TestFieldSetting {
    width: 0,
    visibility: 0,
  });
  HashMap::from([
    (DatabaseLayout::Grid, field_settings.clone()),
    (DatabaseLayout::Board, field_settings),
    (
      DatabaseLayout::Calendar,
      TestFieldSetting {
        width: 0,
        visibility: 0,
      }
      .into(),
    ),
  ])
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
