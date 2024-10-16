use std::future::Future;
use std::ops::{Deref, DerefMut};
use std::sync::{Arc, Weak};
use std::time::Duration;

use async_trait::async_trait;

use collab::preclude::{Collab, CollabBuilder};
use collab_database::database::{gen_database_id, gen_field_id, gen_row_id};
use collab_database::error::DatabaseError;
use collab_database::fields::Field;
use collab_database::rows::{Cells, CreateRowParams, RowId};
use collab_database::views::DatabaseLayout;
use collab_database::workspace_database::{
  DatabaseCollabPersistenceService, DatabaseCollabService, EncodeCollabByOid, RowRelationChange,
  RowRelationUpdateReceiver, WorkspaceDatabaseManager,
};
use collab_entity::CollabType;
use collab_plugins::local_storage::CollabPersistenceConfig;
use tokio::sync::mpsc::{channel, Receiver};

use crate::database_test::helper::field_settings_for_default_database;
use crate::helper::{make_rocks_db, setup_log, TestTextCell};

use collab::core::collab::DataSource;
use collab::core::origin::CollabOrigin;
use collab::entity::EncodedCollab;
use collab::lock::Mutex;
use collab_database::entity::{CreateDatabaseParams, CreateViewParams};
use collab_plugins::local_storage::kv::doc::CollabKVAction;
use collab_plugins::local_storage::kv::KVTransactionDB;
use collab_plugins::local_storage::rocksdb::rocksdb_plugin::RocksdbDiskPlugin;
use collab_plugins::local_storage::rocksdb::util::KVDBCollabPersistenceImpl;
use collab_plugins::CollabKVDB;
use rand::Rng;
use tempfile::TempDir;

pub struct WorkspaceDatabaseTest {
  #[allow(dead_code)]
  uid: i64,
  inner: WorkspaceDatabaseManager,
  pub collab_db: Arc<CollabKVDB>,
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
  rng.gen::<i64>()
}

pub struct TestUserDatabaseServiceImpl {
  pub uid: i64,
  pub db: Arc<CollabKVDB>,
}

pub struct TestUserDatabasePersistenceImpl {
  pub uid: i64,
  pub db: Arc<CollabKVDB>,
}
impl DatabaseCollabPersistenceService for TestUserDatabasePersistenceImpl {
  fn load_collab(&self, collab: &mut Collab) {
    let object_id = collab.object_id().to_string();
    let mut txn = collab.transact_mut();
    let db_read = self.db.read_txn();
    let _ = db_read.load_doc_with_txn(self.uid, &object_id, &mut txn);
  }

  fn get_encoded_collab(&self, object_id: &str, collab_type: CollabType) -> Option<EncodedCollab> {
    let mut collab = Collab::new_with_origin(CollabOrigin::Empty, object_id, vec![], false);
    self.load_collab(&mut collab);
    collab
      .encode_collab_v1(|collab| collab_type.validate_require_data(collab))
      .ok()
  }

  fn delete_collab(&self, object_id: &str) -> Result<(), DatabaseError> {
    let write_txn = self.db.write_txn();
    write_txn.delete_doc(self.uid, object_id).unwrap();
    write_txn.commit_transaction().unwrap();
    Ok(())
  }

  fn save_collab(
    &self,
    object_id: &str,
    encoded_collab: EncodedCollab,
  ) -> Result<(), DatabaseError> {
    let write_txn = self.db.write_txn();
    write_txn
      .flush_doc(
        self.uid,
        &object_id,
        encoded_collab.state_vector.to_vec(),
        encoded_collab.doc_state.to_vec(),
      )
      .map_err(|err| DatabaseError::Internal(err.into()))?;
    write_txn
      .commit_transaction()
      .map_err(|err| DatabaseError::Internal(err.into()))?;
    Ok(())
  }

  fn is_collab_exist(&self, object_id: &str) -> bool {
    let read_txn = self.db.read_txn();
    read_txn.is_exist(self.uid, object_id)
  }

  fn flush_collabs(
    &self,
    encoded_collabs: Vec<(String, EncodedCollab)>,
  ) -> Result<(), DatabaseError> {
    let write_txn = self.db.write_txn();
    for (object_id, encode_collab) in encoded_collabs {
      write_txn
        .flush_doc(
          self.uid,
          &object_id,
          encode_collab.state_vector.to_vec(),
          encode_collab.doc_state.to_vec(),
        )
        .map_err(|e| DatabaseError::Internal(e.into()))?;
    }

    write_txn.commit_transaction().unwrap();
    Ok(())
  }

  fn is_row_exist_partition(&self, row_ids: Vec<RowId>) -> (Vec<RowId>, Vec<RowId>) {
    let read_txn = self.db.read_txn();
    row_ids
      .into_iter()
      .partition(|row_id| read_txn.is_exist(self.uid, row_id.as_ref()))
  }
}

#[async_trait]
impl DatabaseCollabService for TestUserDatabaseServiceImpl {
  async fn build_collab(
    &self,
    object_id: &str,
    object_type: CollabType,
    encoded_collab: Option<(EncodedCollab, bool)>,
  ) -> Result<Collab, DatabaseError> {
    let db_plugin = RocksdbDiskPlugin::new_with_config(
      self.uid,
      object_id.to_string(),
      object_type,
      Arc::downgrade(&self.db),
      CollabPersistenceConfig::default(),
    );

    let data_source = encoded_collab
      .map(|(encoded_collab, _)| DataSource::from(encoded_collab))
      .unwrap_or_else(|| {
        KVDBCollabPersistenceImpl {
          db: Arc::downgrade(&self.db),
          uid: self.uid,
        }
        .into_data_source()
      });

    let mut collab = CollabBuilder::new(self.uid, object_id, data_source)
      .with_device_id("1")
      .with_plugin(db_plugin)
      .build()
      .unwrap();

    collab.initialize();
    Ok(collab)
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
        object_id.to_string(),
        collab_type.clone(),
        Arc::downgrade(&self.db),
        CollabPersistenceConfig::default(),
      );

      let collab = CollabBuilder::new(
        1,
        &object_id,
        KVDBCollabPersistenceImpl {
          db: Arc::downgrade(&self.db),
          uid: self.uid,
        }
        .into_data_source(),
      )
      .with_device_id("1")
      .with_plugin(db_plugin)
      .build()
      .unwrap();

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
      db: self.db.clone(),
    }))
  }
}

pub async fn workspace_database_test(uid: i64) -> WorkspaceDatabaseTest {
  setup_log();
  let db = make_rocks_db();
  let workspace_database = user_database_test_with_db(uid, db).await;
  workspace_database.flush_workspace_database().unwrap();
  workspace_database
}

pub async fn workspace_database_test_with_config(
  uid: i64,
  _config: CollabPersistenceConfig,
) -> WorkspaceDatabaseTest {
  setup_log();
  let collab_db = make_rocks_db();
  let collab_service = TestUserDatabaseServiceImpl {
    uid,
    db: collab_db.clone(),
  };
  let workspace_database_id = uuid::Uuid::new_v4().to_string();
  let collab = collab_service
    .build_collab(&workspace_database_id, CollabType::WorkspaceDatabase, None)
    .await
    .unwrap();
  let inner =
    WorkspaceDatabaseManager::open(&workspace_database_id, collab, collab_service).unwrap();
  WorkspaceDatabaseTest {
    uid,
    inner,
    collab_db,
  }
}

pub async fn workspace_database_with_db(
  uid: i64,
  collab_db: Weak<CollabKVDB>,
  config: Option<CollabPersistenceConfig>,
) -> WorkspaceDatabaseManager {
  let _config = config.unwrap_or_else(|| CollabPersistenceConfig::new().snapshot_per_update(5));
  let builder = TestUserDatabaseServiceImpl {
    uid,
    db: collab_db.clone().upgrade().unwrap(),
  };

  // In test, we use a fixed database_storage_id
  let workspace_database_id = "database_views_aggregate_id";
  let collab = builder
    .build_collab(workspace_database_id, CollabType::WorkspaceDatabase, None)
    .await
    .unwrap();
  WorkspaceDatabaseManager::create(workspace_database_id, collab, builder).unwrap()
}

pub async fn user_database_test_with_db(
  uid: i64,
  collab_db: Arc<CollabKVDB>,
) -> WorkspaceDatabaseTest {
  let inner = workspace_database_with_db(uid, Arc::downgrade(&collab_db), None).await;
  WorkspaceDatabaseTest {
    uid,
    inner,
    collab_db,
  }
}

pub async fn user_database_test_with_default_data(uid: i64) -> WorkspaceDatabaseTest {
  let tempdir = TempDir::new().unwrap();
  let path = tempdir.into_path();
  let db = Arc::new(CollabKVDB::open(path).unwrap());
  let mut w_database = user_database_test_with_db(uid, db).await;

  w_database
    .create_database(create_database_params("d1"))
    .await
    .unwrap();

  w_database
}

fn create_database_params(database_id: &str) -> CreateDatabaseParams {
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
    inline_view_id: view_id.to_string(),
    views: vec![CreateViewParams {
      database_id: database_id.clone(),
      view_id: view_id.to_string(),
      name: name.to_string(),
      layout: DatabaseLayout::Grid,
      field_settings: field_settings_map,
      ..Default::default()
    }],
    rows: vec![
      CreateRowParams::new(gen_row_id(), database_id.clone()),
      CreateRowParams::new(gen_row_id(), database_id.clone()),
      CreateRowParams::new(gen_row_id(), database_id.clone()),
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
