use std::sync::Arc;
use std::time::Duration;

use anyhow::{Error, Result};
use async_trait::async_trait;
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use collab::core::collab::MutexCollab;
use collab::core::origin::CollabOrigin;
use collab_sync::client::sink::{MsgId, SinkConfig, SinkStrategy};
use postgrest::Postgrest;

use crate::cloud_storage::postgres::response::KeyValueListResponse;
use crate::cloud_storage::postgres::SupabaseDBConfig;
use crate::cloud_storage::remote_collab::{CollabObject, RemoteCollab, RemoteCollabStorage};

/// The table must have the following columns:
/// - oid: the object id
/// - key: the key of the update
/// - value: the value of the update
const UPDATE_TABLE_PRIMARY_KEY_COL: &str = "oid";
const UPDATE_TABLE_KEY_COL: &str = "key";
const UPDATE_TABLE_VALUE_COL: &str = "value";

pub struct HttpPostgresDB {
  #[allow(dead_code)]
  postgrest: Arc<Postgrest>,
  remote_collab: Arc<RemoteCollab>,
}

impl HttpPostgresDB {
  pub fn new(object: CollabObject, sync_per_secs: u64, config: SupabaseDBConfig) -> Self {
    let url = format!("{}/rest/v1/", config.url);
    let auth = format!("Bearer {}", config.key);
    let postgrest = Postgrest::new(url)
      .insert_header("apikey", config.key)
      .insert_header("Authorization", auth);
    let postgrest = Arc::new(postgrest);

    let storage = PGCollabCloudStorageImpl {
      postgrest: postgrest.clone(),
      table_name: "collab".to_string(),
      object_id: object.id.clone(),
    };

    let config = SinkConfig::new()
      .with_timeout(15)
      .with_strategy(SinkStrategy::FixInterval(Duration::from_secs(
        sync_per_secs,
      )));

    let remote_collab = Arc::new(RemoteCollab::new(object, storage, config));
    Self {
      postgrest,
      remote_collab,
    }
  }
  /// Start syncing after the local collab is initialized.
  pub async fn start_sync(&self, local_collab: Arc<MutexCollab>) {
    self.remote_collab.sync(local_collab).await;
  }

  pub fn push_update(&self, update: &[u8]) {
    self.remote_collab.push_update(update);
  }
}

struct PGCollabCloudStorageImpl {
  postgrest: Arc<Postgrest>,
  table_name: String,
  object_id: String,
}

#[async_trait]
impl RemoteCollabStorage for PGCollabCloudStorageImpl {
  async fn get_all_updates(&self, object_id: &str) -> Result<Vec<Vec<u8>>, Error> {
    let response = self
      .postgrest
      .from(&self.table_name)
      .eq(UPDATE_TABLE_PRIMARY_KEY_COL, object_id)
      .select(UPDATE_TABLE_VALUE_COL)
      .order(UPDATE_TABLE_KEY_COL)
      .execute()
      .await?;

    if response.status().is_success() {
      let content = response.text().await?;
      let updates = serde_json::from_str::<KeyValueListResponse>(&content)?
        .0
        .into_iter()
        .flat_map(|pair| match STANDARD.decode(pair.value) {
          Ok(data) => Some(data),
          Err(e) => {
            tracing::error!("Failed to decode update from base64 string: {:?}", e);
            None
          },
        })
        .collect::<Vec<Vec<u8>>>();
      Ok(updates)
    } else {
      Err(anyhow::anyhow!("Failed to get all updates: {:?}", response))
    }
  }

  async fn send_update(&self, _id: MsgId, update: Vec<u8>) -> Result<(), Error> {
    tracing::debug!(
      "postgres collab: send update: {}:{:?}",
      self.object_id,
      update.len()
    );
    if update.is_empty() {
      tracing::warn!("🟡Unexpected empty update");
      return Ok(());
    }

    let mut query = serde_json::Map::new();
    let update_str = tokio::task::spawn_blocking(move || STANDARD.encode(update)).await?;
    query.insert(
      UPDATE_TABLE_VALUE_COL.to_string(),
      serde_json::Value::String(update_str),
    );
    query.insert(
      UPDATE_TABLE_PRIMARY_KEY_COL.to_string(),
      serde_json::Value::String(self.object_id.clone()),
    );
    let query_str = serde_json::to_string(&query)?;
    let response = self
      .postgrest
      .from(&self.table_name)
      .eq(UPDATE_TABLE_PRIMARY_KEY_COL, &self.object_id)
      .insert(query_str)
      .execute()
      .await?;

    if response.status().is_success() {
      Ok(())
    } else {
      Err(anyhow::anyhow!("Failed to send update"))
    }
  }
}

pub async fn get_postgres_remote_doc(
  object_id: &str,
  config: SupabaseDBConfig,
) -> Arc<MutexCollab> {
  let object = CollabObject::new(object_id.to_string());
  let local_collab = Arc::new(MutexCollab::new(CollabOrigin::Server, object_id, vec![]));
  let plugin = HttpPostgresDB::new(object, 1, config);
  plugin.start_sync(local_collab.clone()).await;
  local_collab
}
