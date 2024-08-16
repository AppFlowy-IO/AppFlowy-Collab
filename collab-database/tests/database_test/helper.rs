use std::collections::HashMap;
use std::ops::{Deref, DerefMut};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use collab::core::collab::DataSource;
use collab::preclude::CollabBuilder;
use collab_database::database::{Database, DatabaseContext};
use collab_database::fields::Field;
use collab_database::rows::{Cells, CreateRowParams, DatabaseRow, RowId};
use collab_database::views::{
  CreateDatabaseParams, CreateViewParams, DatabaseLayout, FieldSettingsByFieldIdMap,
  FieldSettingsMap, LayoutSetting, LayoutSettings, OrderObjectPosition,
};
use collab_database::workspace_database::DatabaseCollabService;
use collab_entity::CollabType;
use collab_plugins::local_storage::CollabPersistenceConfig;

use crate::helper::{make_rocks_db, setup_log, TestFieldSetting, TestTextCell};
use crate::user_test::helper::TestUserDatabaseCollabBuilderImpl;
use collab_database::database_state::DatabaseNotify;
use collab_plugins::local_storage::rocksdb::util::KVDBCollabPersistenceImpl;
use collab_plugins::CollabKVDB;
use tempfile::TempDir;
use tokio::time::timeout;

pub struct DatabaseTest {
  #[allow(dead_code)]
  collab_db: Arc<CollabKVDB>,
  pub database: Database,
}

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
pub fn create_database(uid: i64, database_id: &str) -> DatabaseTest {
  setup_log();
  let collab_db = make_rocks_db();
  let mut collab = CollabBuilder::new(uid, database_id, DataSource::Disk(None))
    .with_device_id("1")
    .build()
    .unwrap();
  collab.initialize();
  let collab_builder = Arc::new(TestUserDatabaseCollabBuilderImpl());
  let context = DatabaseContext {
    uid,
    db: Arc::downgrade(&collab_db),
    collab,
    collab_service: collab_builder,
    notifier: DatabaseNotify::default(),
  };
  let params = CreateDatabaseParams {
    database_id: database_id.to_string(),
    inline_view_id: "v1".to_string(),
    views: vec![CreateViewParams {
      database_id: database_id.to_string(),
      view_id: "v1".to_string(),
      name: "my first database view".to_string(),
      ..Default::default()
    }],
    ..Default::default()
  };
  let database = Database::new_with_view(params, context).unwrap();
  DatabaseTest {
    database,
    collab_db,
  }
}

pub fn create_row(uid: i64, row_id: RowId) -> DatabaseRow {
  let collab_db = make_rocks_db();
  let mut collab = CollabBuilder::new(uid, row_id.clone(), DataSource::Disk(None))
    .with_device_id("1")
    .build()
    .unwrap();
  collab.initialize();
  let row_change_tx = tokio::sync::broadcast::channel(1).0;
  DatabaseRow::new(
    uid,
    row_id,
    Arc::downgrade(&collab_db),
    collab,
    row_change_tx,
    None,
  )
}

pub async fn create_database_with_db(
  uid: i64,
  database_id: &str,
) -> (Arc<CollabKVDB>, DatabaseTest) {
  setup_log();
  let collab_db = make_rocks_db();
  let collab_builder = Arc::new(TestUserDatabaseCollabBuilderImpl());
  let collab = collab_builder
    .build_collab_with_config(
      uid,
      database_id,
      CollabType::Database,
      Arc::downgrade(&collab_db),
      DataSource::Disk(None),
      CollabPersistenceConfig::default(),
    )
    .unwrap();
  let context = DatabaseContext {
    uid,
    db: Arc::downgrade(&collab_db),
    collab,
    collab_service: collab_builder,
    notifier: DatabaseNotify::default(),
  };
  let params = CreateDatabaseParams {
    database_id: database_id.to_string(),
    inline_view_id: "v1".to_string(),
    views: vec![CreateViewParams {
      database_id: database_id.to_string(),
      view_id: "v1".to_string(),
      name: "my first grid".to_string(),
      ..Default::default()
    }],
    ..Default::default()
  };
  let database = Database::new_with_view(params, context).unwrap();
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
  collab_db: Arc<CollabKVDB>,
) -> DatabaseTest {
  let data_source = KVDBCollabPersistenceImpl {
    db: Arc::downgrade(&collab_db),
    uid,
  };
  let collab_builder = Arc::new(TestUserDatabaseCollabBuilderImpl());
  let collab = collab_builder
    .build_collab_with_config(
      uid,
      database_id,
      CollabType::Database,
      Arc::downgrade(&collab_db),
      data_source.into(),
      CollabPersistenceConfig::default(),
    )
    .unwrap();
  let context = DatabaseContext {
    uid,
    db: Arc::downgrade(&collab_db),
    collab,
    collab_service: collab_builder,
    notifier: DatabaseNotify::default(),
  };
  let database = Database::new(database_id, context).unwrap();
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

  pub fn build(self) -> DatabaseTest {
    let tempdir = TempDir::new().unwrap();
    let path = tempdir.into_path();
    let collab_db = Arc::new(CollabKVDB::open(path).unwrap());
    let data_source = KVDBCollabPersistenceImpl {
      db: Arc::downgrade(&collab_db),
      uid: self.uid,
    }
    .into_data_source();
    let mut collab = CollabBuilder::new(self.uid, &self.database_id, data_source)
      .with_device_id("1")
      .build()
      .unwrap();
    collab.initialize();
    let collab_builder = Arc::new(TestUserDatabaseCollabBuilderImpl());
    let context = DatabaseContext {
      uid: self.uid,
      db: Arc::downgrade(&collab_db),
      collab,
      collab_service: collab_builder,
      notifier: DatabaseNotify::default(),
    };
    let params = CreateDatabaseParams {
      database_id: self.database_id.clone(),
      inline_view_id: self.view_id.clone(),
      views: vec![CreateViewParams {
        database_id: self.database_id,
        view_id: self.view_id,
        name: "my first database view".to_string(),
        layout: self.layout,
        layout_settings: self.layout_settings,
        field_settings: self.field_settings,
        ..Default::default()
      }],
      rows: self.rows,
      fields: self.fields,
    };
    let database = Database::new_with_view(params, context).unwrap();
    DatabaseTest {
      database,
      collab_db,
    }
  }
}

/// Create a database with default data
/// It will create a default view with id 'v1'
pub fn create_database_with_default_data(uid: i64, database_id: &str) -> DatabaseTest {
  let row_1 = CreateRowParams::new(1, database_id.to_string()).with_cells(Cells::from([
    ("f1".into(), TestTextCell::from("1f1cell").into()),
    ("f2".into(), TestTextCell::from("1f2cell").into()),
    ("f3".into(), TestTextCell::from("1f3cell").into()),
  ]));
  let row_2 = CreateRowParams::new(2, database_id.to_string()).with_cells(Cells::from([
    ("f1".into(), TestTextCell::from("2f1cell").into()),
    ("f2".into(), TestTextCell::from("2f2cell").into()),
  ]));
  let row_3 = CreateRowParams::new(3, database_id.to_string()).with_cells(Cells::from([
    ("f1".into(), TestTextCell::from("3f1cell").into()),
    ("f3".into(), TestTextCell::from("3f3cell").into()),
  ]));

  let mut database_test = create_database(uid, database_id);
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
  let field_settings =
    FieldSettingsMap::from([("width".into(), 0.into()), ("visibility".into(), 0.into())]);
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

pub async fn wait_for_specific_event<F, T>(
  mut change_rx: tokio::sync::broadcast::Receiver<T>,
  condition: F,
) -> Result<(), String>
where
  F: Fn(&T) -> bool,
  T: Clone,
{
  loop {
    let result = timeout(Duration::from_secs(5), change_rx.recv()).await;

    match result {
      Ok(Ok(event)) if condition(&event) => {
        // If the event matches the condition
        return Ok(());
      },
      Ok(Ok(_)) => {
        // If it's any other event, continue the loop
        continue;
      },
      Ok(Err(e)) => {
        // Channel error
        return Err(format!("Channel error: {}", e));
      },
      Err(e) => {
        // Timeout occurred
        return Err(format!("Timeout occurred: {}", e));
      },
    }
  }
}
