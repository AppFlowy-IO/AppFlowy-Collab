use crate::disk::script::CollabPersistenceTest;
use collab_plugins::local_storage::CollabPersistenceConfig;
use serde_json::json;
use std::time::Duration;

#[tokio::test]
async fn undo_multiple_insert_test() {
  let mut test = CollabPersistenceTest::new(CollabPersistenceConfig::new());
  let doc_id = "1".to_string();

  // Create and enable undo/redo for the document
  test.create_collab(doc_id.clone()).await;
  test.enable_undo_redo(&doc_id).await;

  // Insert values into the document
  test.insert(&doc_id, "1".to_string(), "a".into()).await;
  test.insert(&doc_id, "2".to_string(), "b".into()).await;
  test.insert(&doc_id, "3".to_string(), "3".into()).await;

  // Assert the current state of the document
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

  // Undo the changes and assert the state is empty
  test.undo(&doc_id).await;
  test.assert_collab(&doc_id, json!({})).await;
}

#[tokio::test]
async fn undo_multiple_insert_test2() {
  let mut test = CollabPersistenceTest::new(CollabPersistenceConfig::new());
  let doc_id = "1".to_string();

  // Create and enable undo/redo for the document
  test.create_collab(doc_id.clone()).await;
  test.enable_undo_redo(&doc_id).await;

  // Insert an initial value into the document
  test.insert(&doc_id, "1".to_string(), "a".into()).await;

  // Wait for 1000 ms to separate the undo grouping
  tokio::time::sleep(Duration::from_millis(1000)).await;
  test.insert(&doc_id, "2".to_string(), "b".into()).await;
  test.insert(&doc_id, "3".to_string(), "3".into()).await;

  // Undo the last insertions and assert the state
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

  // Create and enable undo/redo for the document
  test.create_collab(doc_id.clone()).await;
  test.enable_undo_redo(&doc_id).await;

  // Insert values into the document
  test.insert(&doc_id, "1".to_string(), "a".into()).await;
  test.insert(&doc_id, "2".to_string(), "b".into()).await;

  // Undo and then redo the changes
  test.undo(&doc_id).await;
  test.redo(&doc_id).await;

  // Assert the final state of the document
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
