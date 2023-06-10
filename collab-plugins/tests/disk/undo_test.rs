use std::time::Duration;

use collab_plugins::disk::rocksdb::CollabPersistenceConfig;
use serde_json::json;

use crate::disk::script::CollabPersistenceTest;

#[tokio::test]
async fn undo_multiple_insert_test() {
  let mut test = CollabPersistenceTest::new(CollabPersistenceConfig::new());
  let doc_id = "1".to_string();

  // These are grouped together on time-based ranges (configurable in undo::Options, which is 500ms
  // by default). check out the Collab::new_with_client for more details.
  test.create_collab(doc_id.clone()).await;
  test.insert(&doc_id, "1".to_string(), "a".into()).await;
  test.insert(&doc_id, "2".to_string(), "b".into()).await;
  test.insert(&doc_id, "3".to_string(), "3".into()).await;

  test
    .assert_collab(
      &doc_id,
      json!({
        "1": "a",
        "2": "b",
        "3": "3"
      }),
    )
    .await;

  test.undo(&doc_id).await;
  test.assert_collab(&doc_id, json!({})).await;
}

#[tokio::test]
async fn undo_multiple_insert_test2() {
  let mut test = CollabPersistenceTest::new(CollabPersistenceConfig::new());
  let doc_id = "1".to_string();

  test.create_collab(doc_id.clone()).await;
  test.insert(&doc_id, "1".to_string(), "a".into()).await;

  // Wait for 1000 ms to ensure that the undo is not grouped with the previous insert.
  tokio::time::sleep(Duration::from_millis(1000)).await;
  test.insert(&doc_id, "2".to_string(), "b".into()).await;
  test.insert(&doc_id, "3".to_string(), "3".into()).await;

  test.undo(&doc_id).await;
  test
    .assert_collab(
      &doc_id,
      json!({
        "1": "a"
      }),
    )
    .await;
}

#[tokio::test]
async fn redo_multiple_insert_test2() {
  let mut test = CollabPersistenceTest::new(CollabPersistenceConfig::new());
  let doc_id = "1".to_string();

  test.create_collab(doc_id.clone()).await;
  test.insert(&doc_id, "1".to_string(), "a".into()).await;
  test.insert(&doc_id, "2".to_string(), "b".into()).await;

  test.undo(&doc_id).await;
  test.redo(&doc_id).await;

  test
    .assert_collab(
      &doc_id,
      json!({
        "1": "a",
        "2": "b"
      }),
    )
    .await;
}
