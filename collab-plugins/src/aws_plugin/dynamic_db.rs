use aws_config::meta::region::RegionProviderChain;
use aws_sdk_dynamodb::primitives::Blob;
use aws_sdk_dynamodb::types::{
  AttributeDefinition, AttributeValue, ComparisonOperator, Condition, KeySchemaElement, KeyType,
  ProvisionedThroughput, ScalarAttributeType,
};
use aws_sdk_dynamodb::Client;
use collab::preclude::CollabPlugin;
use parking_lot::Mutex;
use std::mem;
use std::sync::Arc;

use yrs::{merge_updates_v1, ReadTxn, Transaction, TransactionMut};

const DEFAULT_TABLE_NAME: &str = "collab";
const OBJECT_ID: &str = "oid";
const UPDATE_KEY: &str = "k";
const UPDATE_VALUE: &str = "v";

/// A plugin that uses AWS DynamoDB as the backend.
/// https://docs.aws.amazon.com/sdk-for-rust/latest/dg/rust_dynamodb_code_examples.html
/// https://github.com/awsdocs/aws-doc-sdk-examples/tree/main/rust_dev_preview/dynamodb#code-examples
pub struct AWSDynamoDBPlugin {
  object_id: String,
  table_name: String,
  client: Arc<Client>,
  cache: UpdateCache,
}

impl AWSDynamoDBPlugin {
  pub async fn new(object_id: String) -> Result<Self, anyhow::Error> {
    Self::new_with_table_name(object_id, DEFAULT_TABLE_NAME).await
  }

  pub async fn new_with_table_name(
    object_id: String,
    table_name: &str,
  ) -> Result<Self, anyhow::Error> {
    let region_provider = RegionProviderChain::default_provider().or_else("us-east-1");
    let config = aws_config::from_env().region(region_provider).load().await;
    let client = Arc::new(Client::new(&config));
    let table_name = table_name.to_string();

    let (tx, mut rx) = tokio::sync::mpsc::channel(100);
    let cache = UpdateCache::new(10, tx);
    let weak_client = Arc::downgrade(&client);
    let cloned_object_id = object_id.clone();
    let cloned_table_name = table_name.clone();

    tokio::spawn(async move {
      while let Some(update) = rx.recv().await {
        if let Some(client) = weak_client.upgrade() {
          let _ = add_item(&client, &cloned_table_name, &cloned_object_id, 1, update).await;
        }
      }
    });

    let this = Self {
      object_id,
      table_name,
      client,
      cache,
    };
    this.create_table_if_not_exist().await?;
    Ok(this)
  }

  pub async fn add_item<V: Into<Vec<u8>>>(&self, key: i64, value: V) -> Result<(), anyhow::Error> {
    add_item(&self.client, &self.table_name, &self.object_id, key, value).await?;
    Ok(())
  }

  pub async fn get_all_items(&self) -> Vec<Vec<u8>> {
    get_all_items(&self.client, &self.table_name, &self.object_id).await
  }

  pub async fn sync<T: ReadTxn>(&self, txn: &T) {
    let local_sv = txn.state_vector();
  }

  async fn create_table_if_not_exist(&self) -> Result<(), anyhow::Error> {
    let resp = self.client.list_tables().send().await?;
    let tables = resp.table_names().unwrap_or_default();
    tracing::trace!("Current tables: {:?}", tables);
    if tables.contains(&self.table_name) {
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

    let _create_table_response = self
      .client
      .create_table()
      .table_name(self.table_name.clone())
      .key_schema(object_id_ks)
      .key_schema(update_key_ks)
      .attribute_definitions(update_key_ad)
      .attribute_definitions(object_id_ad)
      .provisioned_throughput(pt)
      .send()
      .await?;
    Ok(())
  }
}

#[inline(always)]
async fn add_item<V: Into<Vec<u8>>>(
  client: &Client,
  table_name: &str,
  object_id: &str,
  key: i64,
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

#[inline(always)]
pub async fn get_all_items(client: &Client, table_name: &str, object_id: &str) -> Vec<Vec<u8>> {
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

impl CollabPlugin for AWSDynamoDBPlugin {
  fn receive_update(&self, _object_id: &str, _txn: &TransactionMut, _update: &[u8]) {}
}

type UpdateSender = tokio::sync::mpsc::Sender<Vec<u8>>;
pub(crate) struct UpdateCache {
  merge_if_excess: usize,
  updates: Mutex<Vec<Vec<u8>>>,
  sender: UpdateSender,
}

impl UpdateCache {
  pub(crate) fn new(merge_if_excess: usize, sender: UpdateSender) -> Self {
    Self {
      merge_if_excess,
      updates: Mutex::new(Vec::new()),
      sender,
    }
  }

  pub(crate) fn push(&self, update: Vec<u8>) {
    let mut updates = self.updates.lock();
    updates.push(update);
    let should_merge = updates.len() > self.merge_if_excess;
    if should_merge {
      self.merge(&mut updates);
    }
  }

  pub(crate) fn merge(&self, updates: &mut Vec<Vec<u8>>) {
    let merged_updates = mem::take(updates);
    let updates_ref = merged_updates
      .iter()
      .map(|update| update.as_slice())
      .collect::<Vec<&[u8]>>();

    match merge_updates_v1(updates_ref.as_slice()) {
      Ok(_update) => {
        todo!()
        // match self.sender.send(update) {
        //   Ok(_) => {},
        //   Err(e) => tracing::error!("Send updates failed: {:?}", e),
        // }
      },
      Err(e) => {
        tracing::error!("Merge updates failed: {:?}", e);
        updates.extend(merged_updates);
      },
    }
  }
}

#[cfg(test)]
mod tests {
  use crate::aws_plugin::AWSDynamoDBPlugin;
  #[tokio::test]
  async fn create_table() {
    let plugin = AWSDynamoDBPlugin::new("1".to_string()).await.unwrap();
    plugin.add_item(0, vec![0]).await.unwrap();
    plugin.add_item(1, vec![2]).await.unwrap();
    plugin.add_item(2, vec![1]).await.unwrap();
    plugin.get_all_items().await;
  }
}
