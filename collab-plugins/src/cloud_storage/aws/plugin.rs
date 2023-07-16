use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Weak};
use std::time::Duration;

use collab::core::collab::MutexCollab;
use collab::core::origin::CollabOrigin;
use collab::preclude::CollabPlugin;
use parking_lot::RwLock;
use tokio_retry::strategy::FixedInterval;
use tokio_retry::{Action, Condition, RetryIf};
use y_sync::awareness::Awareness;

use crate::cloud_storage::aws::{AWSDynamoDB, DEFAULT_TABLE_NAME};

enum LoadingState {
  NotLoaded,
  Loading,
  Loaded,
}

/// A plugin that uses AWS DynamoDB as the backend.
pub struct AWSDynamoDBPlugin {
  object_id: String,
  table_name: String,
  region: String,
  sync_per_secs: u64,
  local_collab: Weak<MutexCollab>,
  aws_dynamodb: Arc<RwLock<Option<AWSDynamoDB>>>,
  state: Arc<RwLock<LoadingState>>,
  pending_updates: Arc<RwLock<Vec<Vec<u8>>>>,
}

impl AWSDynamoDBPlugin {
  pub fn new(
    object_id: String,
    local_collab: Weak<MutexCollab>,
    sync_per_secs: u64,
    region: String,
  ) -> Self {
    Self::new_with_table_name(
      object_id,
      DEFAULT_TABLE_NAME,
      local_collab,
      sync_per_secs,
      region,
    )
  }

  pub fn new_with_table_name(
    object_id: String,
    table_name: &str,
    local_collab: Weak<MutexCollab>,
    sync_per_secs: u64,
    region: String,
  ) -> Self {
    let table_name = table_name.to_string();
    let state = Arc::new(RwLock::new(LoadingState::NotLoaded));
    let pending_updates = Arc::new(RwLock::new(Vec::new()));
    Self {
      object_id,
      table_name,
      local_collab,
      sync_per_secs,
      aws_dynamodb: Default::default(),
      region,
      state,
      pending_updates,
    }
  }

  fn init_aws_dynamodb(&self) {
    let retry_strategy = FixedInterval::new(Duration::from_secs(5)).take(10);
    let action = AwsDynamodbConnectAction::new(
      self.object_id.clone(),
      self.table_name.clone(),
      self.region.clone(),
      self.sync_per_secs,
    );

    let weak_local_collab = self.local_collab.clone();
    let weak_aws_dynamodb = Arc::downgrade(&self.aws_dynamodb);
    let weak_state = Arc::downgrade(&self.state);
    let weak_pending_updates = Arc::downgrade(&self.pending_updates);
    tokio::spawn(async move {
      if let Some(aws_dynamodb) = weak_aws_dynamodb.upgrade() {
        if let Ok(dynamodb) = RetryIf::spawn(
          retry_strategy,
          action,
          RetryCondition(weak_aws_dynamodb.clone()),
        )
        .await
        {
          if let (Some(local_collab), Some(state), Some(pending_updates)) = (
            weak_local_collab.upgrade(),
            weak_state.upgrade(),
            weak_pending_updates.upgrade(),
          ) {
            dynamodb.start_sync(local_collab).await;
            for update in &*pending_updates.read() {
              dynamodb.push_update(update);
            }
            *aws_dynamodb.write() = Some(dynamodb);
            *state.write() = LoadingState::Loaded;
          }
        };
      }
    });
  }
}

impl CollabPlugin for AWSDynamoDBPlugin {
  fn did_init(&self, _awareness: &Awareness, _object_id: &str) {
    if matches!(&*self.state.read(), LoadingState::NotLoaded) {
      *self.state.write() = LoadingState::Loading;
      self.init_aws_dynamodb();
    }
  }

  fn receive_local_update(&self, _origin: &CollabOrigin, _object_id: &str, update: &[u8]) {
    if let Some(aws_dynamodb) = self.aws_dynamodb.write().as_ref() {
      aws_dynamodb.push_update(update);
    } else {
      self.pending_updates.write().push(update.to_vec());
    }
  }
}

pub(crate) struct AwsDynamodbConnectAction {
  object_id: String,
  table_name: String,
  region: String,
  sync_per_secs: u64,
}

impl AwsDynamodbConnectAction {
  pub fn new(object_id: String, table_name: String, region: String, sync_per_secs: u64) -> Self {
    Self {
      object_id,
      table_name,
      region,
      sync_per_secs,
    }
  }
}

impl Action for AwsDynamodbConnectAction {
  type Future = Pin<Box<dyn Future<Output = Result<Self::Item, Self::Error>> + Send>>;
  type Item = AWSDynamoDB;
  type Error = anyhow::Error;

  fn run(&mut self) -> Self::Future {
    let cloned_object_id = self.object_id.clone();
    let cloned_table_name = self.table_name.clone();
    let cloned_region = self.region.clone();
    let sync_per_secs = self.sync_per_secs;
    Box::pin(async move {
      AWSDynamoDB::new_with_table_name(
        1,
        cloned_object_id,
        cloned_table_name,
        sync_per_secs,
        cloned_region,
      )
      .await
    })
  }
}

struct RetryCondition(Weak<RwLock<Option<AWSDynamoDB>>>);
impl Condition<anyhow::Error> for RetryCondition {
  fn should_retry(&mut self, _error: &anyhow::Error) -> bool {
    self.0.upgrade().is_some()
  }
}
