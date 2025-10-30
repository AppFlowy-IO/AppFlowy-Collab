use crate::database_test::helper::create_database_with_default_data;
use assert_json_diff::assert_json_eq;
use collab::core::collab::CollabOptions;
use collab::core::origin::CollabOrigin;
use collab::document::blocks::{Block, DocumentData, DocumentMeta};
use collab::document::document::Document;
use collab::entity::{CollabType, EncodedCollab};
use collab::plugins::CollabKVDB;
use collab::plugins::local_storage::rocksdb::rocksdb_plugin::RocksdbDiskPlugin;
use collab::preclude::Collab;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use yrs::block::ClientID;

fn persist_row_document(
  uid: i64,
  workspace_id: &str,
  document_id: &str,
  client_id: ClientID,
  collab_db: Arc<CollabKVDB>,
) -> EncodedCollab {
  fn default_document_data() -> DocumentData {
    let page_id = uuid::Uuid::new_v4().to_string();
    let text_block_id = uuid::Uuid::new_v4().to_string();
    let page_children_id = uuid::Uuid::new_v4().to_string();
    let text_children_id = uuid::Uuid::new_v4().to_string();
    let text_external_id = uuid::Uuid::new_v4().to_string();

    let mut data = HashMap::new();
    data.insert("delta".to_string(), json!([]));

    let mut blocks = HashMap::new();
    blocks.insert(
      page_id.clone(),
      Block {
        id: page_id.clone(),
        ty: "page".to_string(),
        parent: "".to_string(),
        children: page_children_id.clone(),
        external_id: None,
        external_type: None,
        data: data.clone(),
      },
    );
    blocks.insert(
      text_block_id.clone(),
      Block {
        id: text_block_id.clone(),
        ty: "text".to_string(),
        parent: page_id.clone(),
        children: text_children_id.clone(),
        external_id: Some(text_external_id.clone()),
        external_type: Some("text".to_string()),
        data,
      },
    );

    let mut children_map = HashMap::new();
    children_map.insert(page_children_id, vec![text_block_id.clone()]);
    children_map.insert(text_children_id, vec![]);

    let mut text_map = HashMap::new();
    text_map.insert(text_external_id, "[]".to_string());

    DocumentData {
      page_id,
      blocks,
      meta: DocumentMeta {
        children_map,
        text_map: Some(text_map),
      },
    }
  }

  let mut document = Document::create(document_id, default_document_data(), client_id).unwrap();
  document.add_plugin(Box::new(RocksdbDiskPlugin::new(
    uid,
    workspace_id.to_string(),
    document_id.to_string(),
    CollabType::Document,
    Arc::downgrade(&collab_db),
  )));
  document.initialize();
  document.encode_collab().unwrap()
}

#[tokio::test]
async fn encode_database_collab_test() {
  let database_id = uuid::Uuid::new_v4().to_string();
  let mut database_test = create_database_with_default_data(1, &database_id).await;

  // Prepare a persisted row document for the first row so we can validate document encoding.
  let row_id = database_test.pre_define_row_ids[0];
  let document_id = database_test
    .get_row_document_id(&row_id)
    .expect("row document id");
  let expected_document_collab = persist_row_document(
    1,
    &database_test.workspace_id,
    &document_id,
    database_test.client_id,
    database_test.collab_db.clone(),
  );
  database_test
    .update_row_meta(&row_id, |meta| {
      meta.update_is_document_empty(false);
    })
    .await;

  let database_collab = database_test.encode_database_collabs().await.unwrap();
  let collab::database::entity::EncodedDatabase {
    encoded_database_collab: _,
    encoded_row_collabs,
    encoded_row_document_collabs,
  } = database_collab;

  assert_eq!(encoded_row_collabs.len(), 3);
  assert_eq!(encoded_row_document_collabs.len(), 1);

  for (index, encoded_info) in encoded_row_collabs.into_iter().enumerate() {
    let object_id = database_test.pre_define_row_ids[index];
    let options = CollabOptions::new(object_id, database_test.client_id)
      .with_data_source(encoded_info.encoded_collab.into());
    let collab = Collab::new_with_options(CollabOrigin::Empty, options).unwrap();
    let json = collab.to_json_value();
    let expected_json = database_test
      .get_database_row(&object_id)
      .await
      .unwrap()
      .read()
      .await
      .to_json_value();
    assert_json_eq!(json, expected_json);
  }

  let mut encoded_row_document_collabs = encoded_row_document_collabs;
  let encoded_document = encoded_row_document_collabs.pop().unwrap();
  assert_eq!(encoded_document.collab_type, CollabType::Document);
  assert_eq!(encoded_document.object_id.to_string(), document_id);
  assert_eq!(
    encoded_document.encoded_collab.state_vector,
    expected_document_collab.state_vector
  );
  assert_eq!(
    encoded_document.encoded_collab.doc_state,
    expected_document_collab.doc_state
  );
}
