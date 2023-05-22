use std::sync::Arc;
use std::time::Duration;

use anyhow::{Error, Result};
use async_trait::async_trait;
use collab::core::collab::MutexCollab;
use collab_sync::client::sink::{MsgId, SinkConfig, SinkStrategy};
use postgrest::Postgrest;
use serde::{Deserialize, Serialize};

use crate::cloud_storage::remote_collab::{RemoteCollab, RemoteCollabStorage};

pub const SUPABASE_URL: &str = "SUPABASE_URL";
pub const SUPABASE_ANON_KEY: &str = "SUPABASE_ANON_KEY";
pub const SUPABASE_KEY: &str = "SUPABASE_KEY";
pub const SUPABASE_JWT_SECRET: &str = "SUPABASE_JWT_SECRET";
pub const SUPABASE_UPDATE_TABLE_NAME: &str = "SUPABASE_UPDATE_TABLE_NAME";
pub const SUPABASE_UPDATE_TABLE_PKEY: &str = "SUPABASE_UPDATE_TABLE_PKEY";
pub const SUPABASE_UPDATE_TABLE_ENABLE: &str = "SUPABASE_UPDATE_TABLE_ENABLE";

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SupabaseDBConfig {
  /// The url of the supabase server.
  pub url: String,
  /// The key of the supabase server.
  pub key: String,
  /// The secret used to sign the JWT tokens.
  pub jwt_secret: String,
  /// Store the [Collab] updates in the update table.
  pub update_table_config: UpdateTableConfig,
}

impl SupabaseDBConfig {
  pub fn from_env() -> Result<Self, anyhow::Error> {
    Ok(Self {
      url: std::env::var(SUPABASE_URL)?,
      key: std::env::var(SUPABASE_KEY)?,
      jwt_secret: std::env::var(SUPABASE_JWT_SECRET)?,
      update_table_config: UpdateTableConfig::from_env()?,
    })
  }

  pub fn write_env(&self) {
    std::env::set_var(SUPABASE_URL, &self.url);
    std::env::set_var(SUPABASE_KEY, &self.key);
    std::env::set_var(SUPABASE_JWT_SECRET, &self.jwt_secret);
    self.update_table_config.write_env();
  }
}

/// UpdateTable is used to store the updates of the collab object.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UpdateTableConfig {
  pub table_name: String,
  /// Primary key of the table. It's used as the unique identifier of the object.
  pub pkey: String,
  /// Whether to enable the update table.
  /// If it's disabled, the updates will be stored in the object table.
  pub enable: bool,
}

impl UpdateTableConfig {
  pub fn write_env(&self) {
    std::env::set_var(SUPABASE_UPDATE_TABLE_NAME, &self.table_name);
    std::env::set_var(SUPABASE_UPDATE_TABLE_PKEY, &self.pkey);
    std::env::set_var(SUPABASE_UPDATE_TABLE_ENABLE, &self.enable.to_string());
  }

  pub fn from_env() -> Result<Self, anyhow::Error> {
    Ok(Self {
      table_name: std::env::var(SUPABASE_UPDATE_TABLE_NAME)?,
      pkey: std::env::var(SUPABASE_UPDATE_TABLE_PKEY)?,
      enable: std::env::var(SUPABASE_UPDATE_TABLE_ENABLE)?
        .parse::<bool>()
        .unwrap_or(false),
    })
  }
}

pub struct PostgresDB {
  object_id: String,
  postgrest: Arc<Postgrest>,
  remote_collab: Arc<RemoteCollab>,
}

impl PostgresDB {
  pub fn new(object_id: String, sync_per_secs: u64, config: SupabaseDBConfig) -> Self {
    let url = format!("{}/rest/v1/", config.url);
    let auth = format!("Bearer {}", config.key);
    let postgrest = Postgrest::new(url)
      .insert_header("apikey", config.key)
      .insert_header("Authorization", auth);
    let postgrest = Arc::new(postgrest);

    let storage = CollabCloudStorageImpl {
      postgrest: postgrest.clone(),
      table_name: config.update_table_config.table_name.clone(),
      object_id: object_id.clone(),
      pkey: config.update_table_config.pkey.clone(),
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
    let _response = self
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
      let _resp = self
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
