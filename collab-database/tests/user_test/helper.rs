use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use collab::core::collab::CollabOptions;
use collab::core::origin::CollabOrigin;
use collab::preclude::Collab;
use collab_database::database::{gen_database_id, gen_field_id};
use collab_database::error::DatabaseError;
use collab_database::fields::Field;
use collab_database::rows::{CreateRowParams, DatabaseRow, RowId};
use collab_database::views::DatabaseLayout;
use collab_database::workspace_database::{RowRelationChange, RowRelationUpdateReceiver};
use collab_entity::CollabType;
use dashmap::DashMap;
use tokio::sync::mpsc::{Receiver, channel};

use crate::database_test::helper::field_settings_for_default_database;

use collab::entity::EncodedCollab;
use collab::lock::RwLock;
use collab_database::database_trait::{
  DatabaseCollabPersistenceService, DatabaseCollabReader, EncodeCollabByOid,
};
use collab_database::entity::{CreateDatabaseParams, CreateViewParams};
use collab_plugins::CollabKVDB;
use collab_plugins::local_storage::kv::KVTransactionDB;
use collab_plugins::local_storage::kv::doc::CollabKVAction;
use rand::Rng;
use uuid::Uuid;
use yrs::block::ClientID;

pub fn random_uid() -> i64 {
  let mut rng = rand::thread_rng();
  rng.r#gen::<i64>()
}

pub struct TestUserDatabaseServiceImpl {
  uid: i64,
  pub workspace_id: String,
  pub db: Arc<CollabKVDB>,
  pub client_id: ClientID,
  pub cache: Arc<DashMap<RowId, Arc<RwLock<DatabaseRow>>>>,
}

impl TestUserDatabaseServiceImpl {
  pub fn new(uid: i64, workspace_id: String, db: Arc<CollabKVDB>, client_id: ClientID) -> Self {
    Self {
      uid,
      workspace_id,
      db,
      client_id,
      cache: Arc::new(Default::default()),
    }
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

  fn upsert_collab(
    &self,
    object_id: &str,
    encoded_collab: EncodedCollab,
  ) -> Result<(), DatabaseError> {
    let db_write = self.db.write_txn();
    let _ = db_write.upsert_doc_with_doc_state(
      self.uid,
      &self.workspace_id,
      object_id,
      encoded_collab.state_vector.to_vec(),
      encoded_collab.doc_state.to_vec(),
    );
    db_write
      .commit_transaction()
      .map_err(|err| DatabaseError::Internal(err.into()))?;
    Ok(())
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
impl DatabaseCollabReader for TestUserDatabaseServiceImpl {
  async fn reader_client_id(&self) -> ClientID {
    self.client_id
  }

  async fn reader_get_collab(
    &self,
    object_id: &str,
    collab_type: CollabType,
  ) -> Result<EncodedCollab, DatabaseError> {
    let options = CollabOptions::new(object_id.to_string(), self.client_id);
    let mut collab = Collab::new_with_options(CollabOrigin::Empty, options).unwrap();
    let object_id = collab.object_id().to_string();
    let mut txn = collab.transact_mut();
    let db_read = self.db.read_txn();
    let _ = db_read.load_doc_with_txn(self.uid, &self.workspace_id, &object_id, &mut txn);
    drop(txn);

    collab
      .encode_collab_v1(|collab| collab_type.validate_require_data(collab))
      .map_err(|e| e.into())
  }

  async fn reader_batch_get_collabs(
    &self,
    object_ids: Vec<String>,
    collab_type: CollabType,
  ) -> Result<EncodeCollabByOid, DatabaseError> {
    let mut map = EncodeCollabByOid::new();
    for object_id in object_ids {
      let options = CollabOptions::new(object_id.to_string(), self.client_id);
      let mut collab = Collab::new_with_options(CollabOrigin::Empty, options).unwrap();
      let object_id = collab.object_id().to_string();
      let mut txn = collab.transact_mut();
      let db_read = self.db.read_txn();
      let _ = db_read.load_doc_with_txn(self.uid, &self.workspace_id, &object_id, &mut txn);
      drop(txn);

      let encoded_collab = collab
        .encode_collab_v1(|collab| collab_type.validate_require_data(collab))
        .unwrap();
      map.insert(object_id, encoded_collab);
    }
    Ok(map)
  }

  fn reader_persistence(&self) -> Option<Arc<dyn DatabaseCollabPersistenceService>> {
    Some(Arc::new(TestUserDatabasePersistenceImpl {
      uid: self.uid,
      workspace_id: self.workspace_id.clone(),
      db: self.db.clone(),
      client_id: self.client_id,
    }))
  }

  fn database_row_cache(&self) -> Option<Arc<DashMap<RowId, Arc<RwLock<DatabaseRow>>>>> {
    Some(self.cache.clone())
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
