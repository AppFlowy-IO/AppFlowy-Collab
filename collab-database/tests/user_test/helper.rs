use std::future::Future;
use std::ops::{Deref, DerefMut};
use std::sync::{Arc, Weak};
use std::time::Duration;

use async_trait::async_trait;

use collab::core::collab::{CollabOptions, DataSource, default_client_id};
use collab::core::origin::CollabOrigin;
use collab::preclude::Collab;
use collab_database::database::{
  Database, DatabaseBody, DatabaseContext, default_database_collab, gen_database_id, gen_field_id,
};
use collab_database::error::DatabaseError;
use collab_database::fields::Field;
use collab_database::rows::{Cells, CreateRowParams, DatabaseRow, RowChangeSender, RowId};
use collab_database::views::DatabaseLayout;
use collab_database::workspace_database::{
  DatabaseCollabPersistenceService, DatabaseCollabService, DatabaseDataVariant,
  DatabaseRowDataVariant, EncodeCollabByOid, RowRelationChange, RowRelationUpdateReceiver,
  WorkspaceDatabaseManager,
};
use collab_entity::CollabType;
use collab_plugins::local_storage::CollabPersistenceConfig;
use tokio::sync::mpsc::{Receiver, channel};

use crate::database_test::helper::field_settings_for_default_database;
use crate::helper::{TestTextCell, make_rocks_db, setup_log};

use collab::entity::EncodedCollab;
use collab::lock::{Mutex, RwLock};
use collab_database::entity::{CreateDatabaseParams, CreateViewParams};
use collab_plugins::CollabKVDB;
use collab_plugins::local_storage::kv::KVTransactionDB;
use collab_plugins::local_storage::kv::doc::CollabKVAction;
use collab_plugins::local_storage::rocksdb::rocksdb_plugin::RocksdbDiskPlugin;
use collab_plugins::local_storage::rocksdb::util::KVDBCollabPersistenceImpl;
use rand::Rng;
use tempfile::TempDir;
use uuid::Uuid;
use yrs::block::ClientID;

pub struct WorkspaceDatabaseTest {
  #[allow(dead_code)]
  uid: i64,
  pub workspace_id: String,
  inner: WorkspaceDatabaseManager,
  pub collab_db: Arc<CollabKVDB>,
  pub client_id: ClientID,
}

impl Deref for WorkspaceDatabaseTest {
  type Target = WorkspaceDatabaseManager;

  fn deref(&self) -> &Self::Target {
    &self.inner
  }
}

impl DerefMut for WorkspaceDatabaseTest {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.inner
  }
}

pub fn random_uid() -> i64 {
  let mut rng = rand::thread_rng();
  rng.r#gen::<i64>()
}

pub struct TestUserDatabaseServiceImpl {
  pub uid: i64,
  pub workspace_id: String,
  pub db: Arc<CollabKVDB>,
  pub client_id: ClientID,
}

impl TestUserDatabaseServiceImpl {
  fn build_collab(
    &self,
    object_id: &str,
    collab_type: CollabType,
    encoded_collab: Option<EncodedCollab>,
  ) -> Result<Collab, DatabaseError> {
    let db_plugin = RocksdbDiskPlugin::new_with_config(
      self.uid,
      self.workspace_id.clone(),
      object_id.to_string(),
      collab_type,
      Arc::downgrade(&self.db),
      CollabPersistenceConfig::default(),
    );

    let data_source = encoded_collab.map(DataSource::from).unwrap_or_else(|| {
      KVDBCollabPersistenceImpl {
        db: Arc::downgrade(&self.db),
        uid: self.uid,
        workspace_id: self.workspace_id.clone(),
      }
      .into_data_source()
    });

    let options =
      CollabOptions::new(object_id.to_string(), self.client_id).with_data_source(data_source);
    let mut collab = Collab::new_with_options(CollabOrigin::Empty, options).unwrap();
    collab.add_plugin(Box::new(db_plugin));
    collab.initialize();
    Ok(collab)
  }
}

pub struct TestUserDatabasePersistenceImpl {
  pub uid: i64,
  pub workspace_id: String,
  pub db: Arc<CollabKVDB>,
  pub client_id: ClientID,
}
impl DatabaseCollabPersistenceService for TestUserDatabasePersistenceImpl {
  fn load_collab(&self, collab: &mut Collab) {
    let object_id = collab.object_id().to_string();
    let mut txn = collab.transact_mut();
    let db_read = self.db.read_txn();
    let _ = db_read.load_doc_with_txn(self.uid, &self.workspace_id, &object_id, &mut txn);
  }

  fn get_encoded_collab(&self, object_id: &str, collab_type: CollabType) -> Option<EncodedCollab> {
    let options = CollabOptions::new(object_id.to_string(), self.client_id);
    let mut collab = Collab::new_with_options(CollabOrigin::Empty, options).unwrap();
    self.load_collab(&mut collab);
    collab
      .encode_collab_v1(|collab| collab_type.validate_require_data(collab))
      .ok()
  }

  fn delete_collab(&self, object_id: &str) -> Result<(), DatabaseError> {
    let write_txn = self.db.write_txn();
    write_txn
      .delete_doc(self.uid, self.workspace_id.as_str(), object_id)
      .unwrap();
    write_txn.commit_transaction().unwrap();
    Ok(())
  }

  fn is_collab_exist(&self, object_id: &str) -> bool {
    let read_txn = self.db.read_txn();
    read_txn.is_exist(self.uid, self.workspace_id.as_str(), object_id)
  }
}

#[async_trait]
impl DatabaseCollabService for TestUserDatabaseServiceImpl {
  async fn client_id(&self) -> ClientID {
    self.client_id
  }

  async fn build_arc_database(
    &self,
    object_id: &str,
    _is_new: bool,
    data: Option<DatabaseDataVariant>,
    context: DatabaseContext,
  ) -> Result<Arc<RwLock<Database>>, DatabaseError> {
    let database = self.build_database(object_id, false, data, context).await?;
    Ok(Arc::new(RwLock::new(database)))
  }

  async fn build_database(
    &self,
    object_id: &str,
    _is_new: bool,
    data: Option<DatabaseDataVariant>,
    context: DatabaseContext,
  ) -> Result<Database, DatabaseError> {
    let collab_service = context.collab_service.clone();
    let collab = match data {
      None => self.build_collab(object_id, CollabType::Database, None)?,
      Some(data) => match data {
        DatabaseDataVariant::Params(params) => {
          let database_id = params.database_id.clone();
          let collab =
            default_database_collab(&database_id, self.client_id, Some(params), context.clone())
              .await?
              .1;
          self.build_collab(
            object_id,
            CollabType::Database,
            Some(collab.encode_collab_v1(|_| Ok::<_, DatabaseError>(()))?),
          )?
        },
        DatabaseDataVariant::EncodedCollab(data) => {
          self.build_collab(object_id, CollabType::Database, Some(data))?
        },
      },
    };

    let (body, collab) = DatabaseBody::open(collab, context)?;
    Ok(Database {
      collab,
      body,
      collab_service,
    })
  }

  async fn build_arc_database_row(
    &self,
    object_id: &str,
    _is_new: bool,
    data: Option<DatabaseRowDataVariant>,
    sender: Option<RowChangeSender>,
    collab_service: Arc<dyn DatabaseCollabService>,
  ) -> Result<Arc<RwLock<DatabaseRow>>, DatabaseError> {
    let data = data.map(|v| v.into_encode_collab(self.client_id));
    let collab = self.build_collab(object_id, CollabType::DatabaseRow, data)?;
    let database_row = DatabaseRow::open(RowId::from(object_id), collab, sender, collab_service)?;

    Ok(Arc::new(RwLock::new(database_row)))
  }

  async fn build_workspace_database_collab(
    &self,
    object_id: &str,
    encoded_collab: Option<EncodedCollab>,
  ) -> Result<Collab, DatabaseError> {
    self.build_collab(object_id, CollabType::WorkspaceDatabase, encoded_collab)
  }

  async fn get_collabs(
    &self,
    object_ids: Vec<String>,
    collab_type: CollabType,
  ) -> Result<EncodeCollabByOid, DatabaseError> {
    let mut map = EncodeCollabByOid::new();
    for object_id in object_ids {
      let db_plugin = RocksdbDiskPlugin::new_with_config(
        1,
        self.workspace_id.clone(),
        object_id.to_string(),
        collab_type,
        Arc::downgrade(&self.db),
        CollabPersistenceConfig::default(),
      );

      let data_source = KVDBCollabPersistenceImpl {
        db: Arc::downgrade(&self.db),
        uid: self.uid,
        workspace_id: self.workspace_id.clone(),
      }
      .into_data_source();

      let options =
        CollabOptions::new(object_id.to_string(), self.client_id).with_data_source(data_source);
      let mut collab = Collab::new_with_options(CollabOrigin::Empty, options).unwrap();
      collab.add_plugin(Box::new(db_plugin));
      collab.initialize();

      let encoded_collab = collab
        .encode_collab_v1(|_| Ok::<_, DatabaseError>(()))
        .unwrap();
      map.insert(object_id, encoded_collab);
    }
    Ok(map)
  }

  fn persistence(&self) -> Option<Arc<dyn DatabaseCollabPersistenceService>> {
    Some(Arc::new(TestUserDatabasePersistenceImpl {
      uid: self.uid,
      workspace_id: self.workspace_id.clone(),
      db: self.db.clone(),
      client_id: self.client_id,
    }))
  }
}

pub async fn workspace_database_test(uid: i64) -> WorkspaceDatabaseTest {
  let workspace_id = Uuid::new_v4().to_string();
  setup_log();
  let db = make_rocks_db();
  user_database_test_with_db(uid, &workspace_id, db).await
}

pub async fn workspace_database_test_with_config(
  uid: i64,
  workspace_id: String,
  _config: CollabPersistenceConfig,
) -> WorkspaceDatabaseTest {
  setup_log();
  let client_id = default_client_id();
  let collab_db = make_rocks_db();
  let collab_service = TestUserDatabaseServiceImpl {
    uid,
    workspace_id: workspace_id.clone(),
    db: collab_db.clone(),
    client_id,
  };
  let workspace_database_id = uuid::Uuid::new_v4().to_string();
  let collab = collab_service
    .build_workspace_database_collab(&workspace_database_id, None)
    .await
    .unwrap();
  let inner =
    WorkspaceDatabaseManager::open(&workspace_database_id, collab, collab_service).unwrap();
  WorkspaceDatabaseTest {
    uid,
    workspace_id,
    inner,
    collab_db,
    client_id,
  }
}

pub async fn workspace_database_with_db(
  uid: i64,
  workspace_id: &str,
  collab_db: Weak<CollabKVDB>,
  config: Option<CollabPersistenceConfig>,
  client_id: ClientID,
) -> WorkspaceDatabaseManager {
  let _config = config.unwrap_or_else(|| CollabPersistenceConfig::new().snapshot_per_update(5));
  let builder = TestUserDatabaseServiceImpl {
    uid,
    workspace_id: workspace_id.to_string(),
    db: collab_db.clone().upgrade().unwrap(),
    client_id,
  };

  // In test, we use a fixed database_storage_id
  let workspace_database_id = "database_views_aggregate_id";
  let collab = builder
    .build_workspace_database_collab(workspace_database_id, None)
    .await
    .unwrap();
  WorkspaceDatabaseManager::create(workspace_database_id, collab, builder).unwrap()
}

pub async fn user_database_test_with_db(
  uid: i64,
  workspace_id: &str,
  collab_db: Arc<CollabKVDB>,
) -> WorkspaceDatabaseTest {
  let client_id = default_client_id();
  let inner = workspace_database_with_db(
    uid,
    workspace_id,
    Arc::downgrade(&collab_db),
    None,
    client_id,
  )
  .await;
  WorkspaceDatabaseTest {
    uid,
    workspace_id: workspace_id.to_string(),
    inner,
    collab_db,
    client_id,
  }
}

pub async fn user_database_test_with_default_data(uid: i64) -> WorkspaceDatabaseTest {
  let workspace_id = Uuid::new_v4().to_string();
  let tempdir = TempDir::new().unwrap();
  let path = tempdir.into_path();
  let db = Arc::new(CollabKVDB::open(path).unwrap());
  let mut w_database = user_database_test_with_db(uid, &workspace_id, db).await;

  let database_id = Uuid::new_v4().to_string();
  w_database
    .create_database(create_database_params(database_id.as_str()))
    .await
    .unwrap();

  w_database
}

fn create_database_params(database_id: &str) -> CreateDatabaseParams {
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
  let field_1 = Field::new("f1".to_string(), "text field".to_string(), 0, true);
  let field_2 = Field::new("f2".to_string(), "single select field".to_string(), 2, true);
  let field_3 = Field::new("f3".to_string(), "checkbox field".to_string(), 1, true);

  let field_settings_map = field_settings_for_default_database();

  CreateDatabaseParams {
    database_id: database_id.to_string(),
    views: vec![CreateViewParams {
      database_id: database_id.to_string(),
      view_id: "v1".to_string(),
      name: "my first database".to_string(),
      field_settings: field_settings_map,
      ..Default::default()
    }],
    rows: vec![row_1, row_2, row_3],
    fields: vec![field_1, field_2, field_3],
  }
}

pub fn poll_row_relation_rx(mut rx: RowRelationUpdateReceiver) -> Receiver<RowRelationChange> {
  let (tx, ret) = channel(1);
  tokio::spawn(async move {
    let cloned_tx = tx.clone();
    while let Ok(change) = rx.recv().await {
      cloned_tx.send(change).await.unwrap();
    }
  });
  ret
}

pub async fn test_timeout<F: Future>(f: F) -> F::Output {
  tokio::time::timeout(Duration::from_secs(2), f)
    .await
    .unwrap()
}

pub fn make_default_grid(view_id: &str, name: &str) -> CreateDatabaseParams {
  let database_id = gen_database_id();

  let text_field = Field::new(gen_field_id(), "Name".to_string(), 0, true);
  let single_select_field = Field::new(gen_field_id(), "Status".to_string(), 3, false);
  let checkbox_field = Field::new(gen_field_id(), "Done".to_string(), 4, false);

  let field_settings_map = field_settings_for_default_database();

  CreateDatabaseParams {
    database_id: database_id.clone(),
    views: vec![CreateViewParams {
      database_id: database_id.clone(),
      view_id: view_id.to_string(),
      name: name.to_string(),
      layout: DatabaseLayout::Grid,
      field_settings: field_settings_map,
      ..Default::default()
    }],
    rows: vec![
      CreateRowParams::new(Uuid::new_v4(), database_id.clone()),
      CreateRowParams::new(Uuid::new_v4(), database_id.clone()),
      CreateRowParams::new(Uuid::new_v4(), database_id.clone()),
    ],
    fields: vec![text_field, single_select_field, checkbox_field],
  }
}

#[derive(Clone)]
pub struct MutexUserDatabase(Arc<Mutex<WorkspaceDatabaseManager>>);

impl MutexUserDatabase {
  pub fn new(inner: WorkspaceDatabaseManager) -> Self {
    Self(Arc::new(Mutex::from(inner)))
  }
}

impl Deref for MutexUserDatabase {
  type Target = Arc<Mutex<WorkspaceDatabaseManager>>;
  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

unsafe impl Sync for MutexUserDatabase {}

unsafe impl Send for MutexUserDatabase {}
