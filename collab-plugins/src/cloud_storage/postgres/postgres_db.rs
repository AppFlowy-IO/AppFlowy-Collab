use std::sync::Arc;
use std::time::Duration;

use anyhow::{Error, Result};
use async_trait::async_trait;
use collab::core::collab::MutexCollab;
use collab_sync::client::sink::{MsgId, SinkConfig, SinkStrategy};
use postgrest::Postgrest;
use serde::{Deserialize, Serialize};

use crate::cloud_storage::remote_collab::{RemoteCollab, RemoteCollabStorage};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SupabasePostgresDBConfig {
  /// The url of the supabase server.
  pub url: String,
  /// The key of the supabase server.
  pub key: String,
  /// The secret used to sign the JWT tokens.
  pub jwt_secret: String,

  pub table_name: String,

  pub pkey: String,

  pub enable: bool,
}

pub struct PostgresDB {
  object_id: String,
  postgrest: Arc<Postgrest>,
  remote_collab: Arc<RemoteCollab>,
}

impl PostgresDB {
  pub fn new(object_id: String, sync_per_secs: u64, config: SupabasePostgresDBConfig) -> Self {
    let url = format!("{}/rest/v1/", config.url);
    let auth = format!("Bearer {}", config.key);
    let postgrest = Postgrest::new(url)
      .insert_header("apikey", config.key)
      .insert_header("Authorization", auth);
    let postgrest = Arc::new(postgrest);

    let storage = CollabCloudStorageImpl {
      postgrest: postgrest.clone(),
      table_name: config.table_name.clone(),
      object_id: object_id.clone(),
      pkey: config.pkey.clone(),
    };

    let config = SinkConfig::new()
      .with_timeout(15)
      .with_strategy(SinkStrategy::FixInterval(Duration::from_secs(
        sync_per_secs,
      )));

    let remote_collab = Arc::new(RemoteCollab::new(object_id.clone(), storage, config));
    Self {
      object_id,
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

struct CollabCloudStorageImpl {
  postgrest: Arc<Postgrest>,
  table_name: String,
  object_id: String,
  pkey: String,
}

#[async_trait]
impl RemoteCollabStorage for CollabCloudStorageImpl {
  async fn get_all_updates(&self, object_id: &str) -> Result<Vec<Vec<u8>>, Error> {
    let response = self
      .postgrest
      .from(&self.table_name)
      .eq(&self.pkey, object_id)
      .select("key,value")
      .order("key")
      .execute()
      .await?;
    todo!()
  }

  async fn send_update(&self, _id: MsgId, update: Vec<u8>) -> Result<(), Error> {
    let mut query = serde_json::Map::new();
    if let Ok(value_str) = String::from_utf8(update) {
      query.insert(self.pkey.clone(), serde_json::Value::String(value_str));
      let query_str = serde_json::to_string(&query)?;
      let resp = self
        .postgrest
        .from(&self.table_name)
        .insert(query_str)
        .execute()
        .await?;
    } else {
      tracing::error!("Failed to convert update to string");
    }
    Ok(())
  }

  async fn flush(&self, _object_id: &str) {
    todo!()
  }
}
