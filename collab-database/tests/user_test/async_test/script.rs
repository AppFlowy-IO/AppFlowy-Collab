use std::path::PathBuf;
use std::sync::Arc;

use assert_json_diff::assert_json_include;
use collab_database::fields::Field;
use collab_database::rows::CreateRowParams;
use collab_database::rows::{Cells, CellsBuilder, RowId};
use collab_database::user::WorkspaceDatabase;
use collab_database::views::{CreateDatabaseParams, CreateViewParams};
use collab_plugins::local_storage::kv::doc::CollabKVAction;
use collab_plugins::local_storage::kv::KVTransactionDB;
use collab_plugins::local_storage::CollabPersistenceConfig;
use collab_plugins::CollabKVDB;
use serde_json::Value;
use tempfile::TempDir;

use crate::database_test::helper::field_settings_for_default_database;
use crate::helper::TestTextCell;
use crate::user_test::helper::workspace_database_with_db;

pub enum DatabaseScript {
  CreateDatabase {
    params: CreateDatabaseParams,
  },
  OpenDatabase {
    database_id: String,
  },
  CloseDatabase {
    database_id: String,
  },
  CreateRow {
    database_id: String,
    params: CreateRowParams,
  },
  EditRow {
    database_id: String,
    row_id: RowId,
    cells: Cells,
  },
  AssertDatabaseInDisk {
    database_id: String,
    expected: Value,
  },
  AssertDatabase {
    database_id: String,
    expected: Value,
  },
  AssertNumOfUpdates {
    oid: String,
    expected: usize,
  },
  IsExist {
    oid: String,
    expected: bool,
  },
}

#[derive(Clone)]
pub struct DatabaseTest {
  pub collab_db: Arc<CollabKVDB>,
  pub db_path: PathBuf,
  pub workspace_database: Arc<WorkspaceDatabase>,
  pub config: CollabPersistenceConfig,
}

pub async fn database_test(config: CollabPersistenceConfig) -> DatabaseTest {
  DatabaseTest::new(config).await
}

impl DatabaseTest {
  pub async fn new(config: CollabPersistenceConfig) -> Self {
    let tempdir = TempDir::new().unwrap();
    let db_path = tempdir.into_path();
    let collab_db = Arc::new(CollabKVDB::open(db_path.clone()).unwrap());
    let workspace_database =
      workspace_database_with_db(1, Arc::downgrade(&collab_db), Some(config.clone())).await;
    Self {
      collab_db,
      workspace_database: Arc::new(workspace_database),
      db_path,
      config,
    }
  }

  pub async fn run_scripts(&mut self, scripts: Vec<DatabaseScript>) {
    let mut handles = vec![];
    for script in scripts {
      let workspace_database = self.workspace_database.clone();
      let db = self.collab_db.clone();
      let config = self.config.clone();
      let handle = tokio::spawn(async move {
        run_script(workspace_database, db, config, script).await;
      });
      handles.push(handle);
    }
    for result in futures::future::join_all(handles).await {
      assert!(result.is_ok());
    }
  }
}

pub async fn run_script(
  workspace_database: Arc<WorkspaceDatabase>,
  db: Arc<CollabKVDB>,
  config: CollabPersistenceConfig,
  script: DatabaseScript,
) {
  match script {
    DatabaseScript::CreateDatabase { params } => {
      workspace_database.create_database(params).unwrap();
    },
    DatabaseScript::OpenDatabase { database_id } => {
      workspace_database.get_database(&database_id).await.unwrap();
    },
    DatabaseScript::CloseDatabase { database_id } => {
      workspace_database.close_database(&database_id);
    },
    DatabaseScript::CreateRow {
      database_id,
      params,
    } => {
      workspace_database
        .get_database(&database_id)
        .await
        .unwrap()
        .lock()
        .create_row(params)
        .unwrap();
    },
    DatabaseScript::EditRow {
      database_id,
      row_id,
      cells,
    } => {
      workspace_database
        .get_database(&database_id)
        .await
        .unwrap()
        .lock()
        .update_row(&row_id, |row| {
          row.set_cells(cells);
        });
    },
    DatabaseScript::AssertDatabaseInDisk {
      database_id,
      expected,
    } => {
      let w_database =
        workspace_database_with_db(1, Arc::downgrade(&db), Some(config.clone())).await;
      let database = w_database.get_database(&database_id).await.unwrap();

      let actual = database.lock().to_json_value();

      assert_json_include!(actual: actual, expected: expected);
    },
    DatabaseScript::AssertDatabase {
      database_id,
      expected,
    } => {
      let database = workspace_database.get_database(&database_id).await.unwrap();
      let actual = database.lock().to_json_value();
      assert_json_diff::assert_json_include!(actual: actual, expected: expected);
    },
    DatabaseScript::IsExist {
      oid: database_id,
      expected,
    } => {
      assert_eq!(db.read_txn().is_exist(1, &database_id), expected,)
    },
    DatabaseScript::AssertNumOfUpdates {
      oid: database_id,
      expected,
    } => {
      let updates = db
        .read_txn()
        .get_decoded_v1_updates(1, &database_id)
        .unwrap();
      assert_eq!(updates.len(), expected,);
    },
  }
}

pub(crate) fn create_database(database_id: &str) -> CreateDatabaseParams {
  let row_1 = CreateRowParams {
    id: 1.into(),
    cells: CellsBuilder::new()
      .insert_cell("f1", TestTextCell::from("1f1cell"))
      .insert_cell("f2", TestTextCell::from("1f2cell"))
      .insert_cell("f3", TestTextCell::from("1f3cell"))
      .build(),
    height: 0,
    created_at: 1703772730,
    modified_at: 1703772762,
    ..Default::default()
  };
  let row_2 = CreateRowParams {
    id: 2.into(),
    cells: CellsBuilder::new()
      .insert_cell("f1", TestTextCell::from("2f1cell"))
      .insert_cell("f2", TestTextCell::from("2f2cell"))
      .build(),
    height: 0,
    created_at: 1703772730,
    modified_at: 1703772762,
    ..Default::default()
  };
  let row_3 = CreateRowParams {
    id: 3.into(),
    cells: CellsBuilder::new()
      .insert_cell("f1", TestTextCell::from("3f1cell"))
      .insert_cell("f3", TestTextCell::from("3f3cell"))
      .build(),
    height: 0,
    created_at: 1703772730,
    modified_at: 1703772762,
    ..Default::default()
  };
  let field_1 = Field::new("f1".to_string(), "text field".to_string(), 0, true);
  let field_2 = Field::new("f2".to_string(), "single select field".to_string(), 2, true);
  let field_3 = Field::new("f3".to_string(), "checkbox field".to_string(), 1, true);

  let field_settings_map = field_settings_for_default_database();

  CreateDatabaseParams {
    database_id: database_id.to_string(),
    inline_view_id: "v1".to_string(),
    views: vec![CreateViewParams {
      database_id: database_id.to_string(),
      view_id: "v1".to_string(),
      name: "my first database view".to_string(),
      field_settings: field_settings_map,
      ..Default::default()
    }],
    rows: vec![row_1, row_2, row_3],
    fields: vec![field_1, field_2, field_3],
  }
}
