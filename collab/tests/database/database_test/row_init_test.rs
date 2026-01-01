use std::sync::{
  Arc,
  atomic::{AtomicUsize, Ordering},
};
use std::time::Duration;

use async_trait::async_trait;
use collab::core::collab::default_client_id;
use collab::database::database::{Database, DatabaseContext};
use collab::database::database_trait::{
  DatabaseCollabReader, DatabaseRowCollabService, DatabaseRowCollabServiceAdapter,
  EncodeCollabByOid,
};
use collab::database::entity::{CreateDatabaseParams, CreateViewParams};
use collab::database::rows::{Row, default_database_row_from_row};
use collab::database::views::DatabaseLayout;
use collab::entity::CollabType;
use collab::entity::uuid_validation::{DatabaseId, ObjectId};
use collab::error::CollabError;
use dashmap::DashMap;
use tokio::time::sleep;
use uuid::Uuid;
use yrs::block::ClientID;

struct CountingRowService {
  client_id: ClientID,
  database_id: DatabaseId,
  build_count: Arc<AtomicUsize>,
  delay: Duration,
}

impl CountingRowService {
  fn new(database_id: DatabaseId, build_count: Arc<AtomicUsize>, delay: Duration) -> Self {
    Self {
      client_id: default_client_id(),
      database_id,
      build_count,
      delay,
    }
  }
}

#[async_trait]
impl DatabaseCollabReader for CountingRowService {
  async fn reader_client_id(&self) -> ClientID {
    self.client_id
  }

  async fn reader_get_collab(
    &self,
    object_id: &ObjectId,
    collab_type: CollabType,
  ) -> Result<collab::entity::EncodedCollab, CollabError> {
    if collab_type != CollabType::DatabaseRow {
      return Err(CollabError::Internal(anyhow::anyhow!(
        "unsupported collab type: {collab_type:?}"
      )));
    }

    self.build_count.fetch_add(1, Ordering::SeqCst);
    if !self.delay.is_zero() {
      sleep(self.delay).await;
    }

    let row_id = *object_id;
    let row = Row::new(row_id, self.database_id);
    Ok(default_database_row_from_row(row, self.client_id))
  }

  async fn reader_batch_get_collabs(
    &self,
    object_ids: Vec<ObjectId>,
    collab_type: CollabType,
  ) -> Result<EncodeCollabByOid, CollabError> {
    let mut map = EncodeCollabByOid::new();
    for object_id in object_ids {
      let encoded = self.reader_get_collab(&object_id, collab_type).await?;
      map.insert(object_id, encoded);
    }
    Ok(map)
  }
}

async fn create_database(
  collab_service: Arc<CountingRowService>,
  row_service: Arc<dyn DatabaseRowCollabService>,
) -> Database {
  let database_id = collab_service.database_id;
  let view_id = Uuid::new_v4();
  let params = CreateDatabaseParams {
    database_id,
    views: vec![CreateViewParams::new(
      database_id,
      view_id,
      "row init test".to_string(),
      DatabaseLayout::Grid,
    )],
    ..Default::default()
  };

  let context = DatabaseContext::new(collab_service, row_service);
  Database::create_with_view(params, context)
    .await
    .expect("create database")
}

#[tokio::test]
async fn cache_only_vs_init_on_demand() {
  let database_id = Uuid::new_v4();
  let build_count = Arc::new(AtomicUsize::new(0));
  let row_cache = Arc::new(DashMap::new());
  let service = Arc::new(CountingRowService::new(
    database_id,
    build_count.clone(),
    Duration::from_millis(0),
  ));
  let row_service = Arc::new(DatabaseRowCollabServiceAdapter::new_with_cache(
    service.clone(),
    row_cache,
  ));
  let database = create_database(service, row_service).await;
  let row_id = Uuid::new_v4();

  assert!(database.get_cached_database_row(&row_id).is_none());
  assert_eq!(build_count.load(Ordering::SeqCst), 0);

  let row = database.get_database_row(&row_id).await;
  assert!(row.is_some());
  assert_eq!(build_count.load(Ordering::SeqCst), 1);
  assert!(database.get_cached_database_row(&row_id).is_some());
}

#[tokio::test]
async fn concurrent_get_or_init_dedupes() {
  let database_id = Uuid::new_v4();
  let build_count = Arc::new(AtomicUsize::new(0));
  let row_cache = Arc::new(DashMap::new());
  let service = Arc::new(CountingRowService::new(
    database_id,
    build_count.clone(),
    Duration::from_millis(50),
  ));
  let row_service = Arc::new(DatabaseRowCollabServiceAdapter::new_with_cache(
    service.clone(),
    row_cache,
  ));
  let database = create_database(service, row_service).await;
  let row_id = Uuid::new_v4();

  let (row_a, row_b) = tokio::join!(
    database.body.block.get_or_init_database_row(&row_id),
    database.body.block.get_or_init_database_row(&row_id)
  );

  let row_a = row_a.expect("row init a");
  let row_b = row_b.expect("row init b");
  assert!(Arc::ptr_eq(&row_a, &row_b));
  assert_eq!(build_count.load(Ordering::SeqCst), 1);
}
