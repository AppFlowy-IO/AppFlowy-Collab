use std::sync::Arc;
use std::time::Duration;

use anyhow::Error;
use async_trait::async_trait;
use aws_config::meta::region::RegionProviderChain;
use aws_sdk_dynamodb::config::Region;
use aws_sdk_dynamodb::primitives::Blob;
use aws_sdk_dynamodb::types::{
  AttributeDefinition, AttributeValue, ComparisonOperator, Condition, KeySchemaElement, KeyType,
  ProvisionedThroughput, ScalarAttributeType,
};
use aws_sdk_dynamodb::Client;
use collab::core::collab::MutexCollab;
use collab::core::origin::CollabOrigin;
use collab_sync::client::sink::{MsgId, SinkConfig, SinkStrategy};

use crate::cloud_storage::remote_collab::{CollabObject, RemoteCollab, RemoteCollabStorage};

pub(crate) const DEFAULT_TABLE_NAME: &str = "collab_test";
const OBJECT_ID: &str = "oid";
const UPDATE_KEY: &str = "key";
const UPDATE_VALUE: &str = "value";

/// A plugin that uses AWS DynamoDB as the backend.
/// https://docs.aws.amazon.com/sdk-for-rust/latest/dg/rust_dynamodb_code_examples.html
pub struct AWSDynamoDB {
  #[allow(dead_code)]
  client: Arc<Client>,
  remote_collab: Arc<RemoteCollab>,
}

impl AWSDynamoDB {
  pub async fn new(
    object_id: String,
    sync_per_secs: u64,
    region: String,
  ) -> Result<Self, anyhow::Error> {
    Self::new_with_table_name(
      object_id,
      DEFAULT_TABLE_NAME.to_string(),
      sync_per_secs,
      region,
    )
    .await
  }

  pub async fn new_with_table_name(
    object_id: String,
    table_name: String,
    sync_per_secs: u64,
    region: String,
  ) -> Result<Self, anyhow::Error> {
    let region_provider = RegionProviderChain::default_provider().or_else(Region::new(region));
    let config = aws_config::from_env().region(region_provider).load().await;
    let client = Arc::new(Client::new(&config));
    let table_name = table_name.to_string();
    create_table_if_not_exist(&client, &table_name).await?;

    let storage = Arc::new(AWSCollabCloudStorageImpl {
      client: client.clone(),
      table_name: table_name.clone(),
      object_id: object_id.clone(),
    });

    let config = SinkConfig::new()
      .with_timeout(10)
      .with_strategy(SinkStrategy::FixInterval(Duration::from_secs(
        sync_per_secs,
      )));

    let object = CollabObject::new(object_id.clone());
    let remote_collab = Arc::new(RemoteCollab::new(object, storage, config));
    Ok(Self {
      client,
      remote_collab,
    })
  }

  /// Start syncing after the local collab is initialized.
  pub async fn start_sync(&self, local_collab: Arc<MutexCollab>) {
    self.remote_collab.sync(local_collab).await;
  }

  pub fn push_update(&self, update: &[u8]) {
    self.remote_collab.push_update(update);
  }
}

struct AWSCollabCloudStorageImpl {
  client: Arc<Client>,
  table_name: String,
  object_id: String,
}

#[async_trait]
impl RemoteCollabStorage for AWSCollabCloudStorageImpl {
  async fn get_all_updates(&self, object_id: &str) -> Result<Vec<Vec<u8>>, Error> {
    Ok(aws_get_all_updates(&self.client, &self.table_name, object_id).await)
  }

  async fn send_update(
    &self,
    _object: &CollabObject,
    msg_id: MsgId,
    update: Vec<u8>,
  ) -> Result<(), Error> {
    tracing::debug!("aws: send_update: {:?}", update);
    aws_send_update(
      &self.client,
      &self.table_name,
      &self.object_id,
      msg_id,
      update,
    )
    .await?;
    Ok(())
  }

  async fn send_init_sync(
    &self,
    _object: &CollabObject,
    _id: MsgId,
    _init_update: Vec<u8>,
  ) -> Result<(), Error> {
    todo!()
  }
}

pub async fn get_aws_remote_doc(object_id: &str, region: String) -> Arc<MutexCollab> {
  let local_collab = Arc::new(MutexCollab::new(CollabOrigin::Server, object_id, vec![]));
  let plugin = AWSDynamoDB::new(object_id.to_string(), 1, region)
    .await
    .unwrap();
  plugin.start_sync(local_collab.clone()).await;
  local_collab
}

#[inline(always)]
async fn aws_send_update<V: Into<Vec<u8>>>(
  client: &Client,
  table_name: &str,
  object_id: &str,
  key: MsgId,
  value: V,
) -> Result<(), anyhow::Error> {
  let object_id = AttributeValue::S(object_id.to_string());
  let key = AttributeValue::N(key.to_string());
  let value = AttributeValue::B(Blob::new(value));
  let request = client
    .put_item()
    .table_name(table_name)
    .item(OBJECT_ID, object_id)
    .item(UPDATE_KEY, key)
    .item(UPDATE_VALUE, value);
  let _ = request.send().await?;
  Ok(())
}

/// https://github.com/awsdocs/aws-doc-sdk-examples/tree/main/rust_dev_preview/dynamodb#code-examples
#[inline(always)]
pub async fn aws_get_all_updates(
  client: &Client,
  table_name: &str,
  object_id: &str,
) -> Vec<Vec<u8>> {
  let values = client
    .query()
    .table_name(table_name)
    .key_conditions(
      OBJECT_ID,
      Condition::builder()
        .comparison_operator(ComparisonOperator::Eq)
        .attribute_value_list(AttributeValue::S(object_id.to_string()))
        .build(),
    )
    .send()
    .await
    .unwrap();

  values
    .items()
    .into_iter()
    .flatten()
    .flat_map(|value| {
      if let Some(AttributeValue::B(b)) = value.get(UPDATE_VALUE) {
        Some(b.clone().into_inner())
      } else {
        None
      }
    })
    .collect()
}

async fn create_table_if_not_exist(client: &Client, table_name: &str) -> Result<(), anyhow::Error> {
  let table_name = table_name.to_string();
  let resp = client.list_tables().send().await?;
  let tables = resp.table_names().unwrap_or_default();
  tracing::trace!("Current tables: {:?}", tables);
  if tables.contains(&table_name) {
    return Ok(());
  }

  let object_id_ks = KeySchemaElement::builder()
    .attribute_name(OBJECT_ID)
    .key_type(KeyType::Hash)
    .build();

  let object_id_ad = AttributeDefinition::builder()
    .attribute_name(OBJECT_ID)
    .attribute_type(ScalarAttributeType::S)
    .build();

  let update_key_ks = KeySchemaElement::builder()
    .attribute_name(UPDATE_KEY)
    .key_type(KeyType::Range)
    .build();

  let update_key_ad = AttributeDefinition::builder()
    .attribute_name(UPDATE_KEY)
    .attribute_type(ScalarAttributeType::N)
    .build();

  let pt = ProvisionedThroughput::builder()
    .read_capacity_units(10)
    .write_capacity_units(5)
    .build();

  let _create_table_response = client
    .create_table()
    .table_name(table_name)
    .key_schema(object_id_ks)
    .key_schema(update_key_ks)
    .attribute_definitions(update_key_ad)
    .attribute_definitions(object_id_ad)
    .provisioned_throughput(pt)
    .send()
    .await?;
  Ok(())
}
