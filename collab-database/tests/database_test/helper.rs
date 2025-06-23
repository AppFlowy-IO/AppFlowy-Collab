use collab::core::collab::{CollabOptions, default_client_id};
use collab::core::origin::CollabOrigin;
use collab::preclude::Collab;
use collab_database::database::{Database, DatabaseContext};
use collab_database::fields::Field;
use collab_database::rows::{Cells, CreateRowParams, DatabaseRow, Row, RowId};
use collab_database::views::{
  DatabaseLayout, FieldSettingsByFieldIdMap, FieldSettingsMap, LayoutSetting, LayoutSettings,
  OrderObjectPosition,
};
use futures::StreamExt;
use std::collections::HashMap;
use std::ops::{Deref, DerefMut};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use crate::helper::{TestFieldSetting, TestTextCell, make_rocks_db, setup_log};
use crate::user_test::helper::TestUserDatabaseServiceImpl;
use collab_database::entity::{CreateDatabaseParams, CreateViewParams};

use collab_plugins::CollabKVDB;
use tempfile::TempDir;
use tokio::time::timeout;
use uuid::Uuid;
use yrs::block::ClientID;

pub struct DatabaseTest {
  pub workspace_id: String,
  pub collab_db: Arc<CollabKVDB>,
  pub database: Database,
  pub pre_define_row_ids: Vec<RowId>,
  pub client_id: ClientID,
}

impl DatabaseTest {
  pub async fn get_rows_for_view(&self, view_id: &str) -> Vec<Row> {
    let rows_stream = self
      .database
      .get_rows_for_view(view_id, 10, None, false)
      .await;
    let rows: Vec<Row> = rows_stream
      .filter_map(|result| async move { result.ok() })
      .collect()
      .await;
    rows
  }
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

pub async fn create_database_with_params(params: CreateDatabaseParams) -> DatabaseTest {
  let uid = 1; // Default user id for testing
  let client_id = default_client_id();
  let workspace_id = Uuid::new_v4().to_string();
  setup_log();
  let collab_db = make_rocks_db();
  let collab_service = Arc::new(TestUserDatabaseServiceImpl::new(
    uid,
    workspace_id.clone(),
    collab_db.clone(),
    client_id,
  ));

  let context = DatabaseContext::new(collab_service.clone(), collab_service);
  let database = Database::create_with_view(params, context).await.unwrap();

  DatabaseTest {
    workspace_id,
    database,
    collab_db,
    pre_define_row_ids: vec![],
    client_id,
  }
}

/// Create a database with a single view.
pub fn create_database(_uid: i64, database_id: &str) -> DatabaseTest {
  let params = CreateDatabaseParams {
    database_id: database_id.to_string(),
    views: vec![CreateViewParams {
      database_id: database_id.to_string(),
      view_id: "v1".to_string(),
      name: "my first database view".to_string(),
      ..Default::default()
    }],
    ..Default::default()
  };

  futures::executor::block_on(async { create_database_with_params(params).await })
}

pub fn create_row(
  _uid: i64,
  _workspace_id: &str,
  row_id: RowId,
  client_id: ClientID,
) -> DatabaseRow {
  let options = CollabOptions::new(row_id.to_string(), client_id);
  let mut collab = Collab::new_with_options(CollabOrigin::Empty, options).unwrap();
  collab.initialize();
  let database_id = Uuid::new_v4().to_string();
  let row_change_tx = tokio::sync::broadcast::channel(1).0;
  DatabaseRow::create(
    row_id.clone(),
    collab,
    Some(row_change_tx),
    Row::new(row_id, &database_id),
  )
}

pub async fn create_database_with_db(
  uid: i64,
  workspace_id: &str,
  database_id: &str,
) -> (Arc<CollabKVDB>, DatabaseTest) {
  setup_log();
  let client_id = default_client_id();
  let collab_db = make_rocks_db();
  let collab_service = Arc::new(TestUserDatabaseServiceImpl::new(
    uid,
    workspace_id.to_string(),
    collab_db.clone(),
    client_id,
  ));
  let context = DatabaseContext::new(collab_service.clone(), collab_service);
  let params = CreateDatabaseParams {
    database_id: database_id.to_string(),
    views: vec![CreateViewParams {
      database_id: database_id.to_string(),
      view_id: "v1".to_string(),
      name: "my first grid".to_string(),
      ..Default::default()
    }],
    ..Default::default()
  };
  let database = Database::create_with_view(params, context).await.unwrap();
  (
    collab_db.clone(),
    DatabaseTest {
      workspace_id: workspace_id.to_string(),
      database,
      collab_db,
      pre_define_row_ids: vec![],
      client_id,
    },
  )
}

pub async fn restore_database_from_db(
  uid: i64,
  workspace_id: &str,
  database_id: &str,
  collab_db: Arc<CollabKVDB>,
) -> DatabaseTest {
  let client_id = default_client_id();
  let collab_service = Arc::new(TestUserDatabaseServiceImpl::new(
    uid,
    workspace_id.to_string(),
    collab_db.clone(),
    client_id,
  ));

  let context = DatabaseContext::new(collab_service.clone(), collab_service);
  let database = Database::open(database_id, context).await.unwrap();
  DatabaseTest {
    workspace_id: workspace_id.to_string(),
    database,
    collab_db,
    pre_define_row_ids: vec![],
    client_id,
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
    let client_id = default_client_id();
    let workspace_id = Uuid::new_v4().to_string();
    let tempdir = TempDir::new().unwrap();
    let path = tempdir.into_path();
    let collab_db = Arc::new(CollabKVDB::open(path).unwrap());
    let collab_service = Arc::new(TestUserDatabaseServiceImpl::new(
      self.uid,
      workspace_id.clone(),
      collab_db.clone(),
      client_id,
    ));
    let context = DatabaseContext::new(collab_service.clone(), collab_service);
    let params = CreateDatabaseParams {
      database_id: self.database_id.clone(),
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
    let database = Database::create_with_view(params, context).await.unwrap();
    DatabaseTest {
      workspace_id,
      database,
      collab_db,
      pre_define_row_ids: vec![],
      client_id,
    }
  }
}

/// Create a database with default data
/// It will create a default view with id 'v1'
pub async fn create_database_with_default_data(uid: i64, database_id: &str) -> DatabaseTest {
  let row_1_id = Uuid::new_v4();
  let row_2_id = Uuid::new_v4();
  let row_3_id = Uuid::new_v4();

  let row_1 = CreateRowParams::new(row_1_id, database_id.to_string()).with_cells(Cells::from([
    ("f1".into(), TestTextCell::from("1f1cell").into()),
    ("f2".into(), TestTextCell::from("1f2cell").into()),
    ("f3".into(), TestTextCell::from("1f3cell").into()),
  ]));
  let row_2 = CreateRowParams::new(row_2_id, database_id.to_string()).with_cells(Cells::from([
    ("f1".into(), TestTextCell::from("2f1cell").into()),
    ("f2".into(), TestTextCell::from("2f2cell").into()),
  ]));
  let row_3 = CreateRowParams::new(row_3_id, database_id.to_string()).with_cells(Cells::from([
    ("f1".into(), TestTextCell::from("3f1cell").into()),
    ("f3".into(), TestTextCell::from("3f3cell").into()),
  ]));

  let mut database_test = create_database(uid, database_id);
  database_test.pre_define_row_ids = vec![row_1.id.clone(), row_2.id.clone(), row_3.id.clone()];
  database_test.create_row(row_1).await.unwrap();
  database_test.create_row(row_2).await.unwrap();
  database_test.create_row(row_3).await.unwrap();

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
  mut condition: F,
) -> Result<(), String>
where
  F: FnMut(&T) -> bool,
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
