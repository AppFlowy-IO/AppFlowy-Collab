use crate::disk::script::CollabPersistenceTest;
use collab_plugins::local_storage::CollabPersistenceConfig;

#[tokio::test]
async fn delete_single_doc_test() {
  let mut test = CollabPersistenceTest::new(CollabPersistenceConfig::default());
  let doc_id = "1".to_string();

  // Replacing Script variants with function calls
  test
    .create_document_with_collab_db(doc_id.clone(), test.db.clone())
    .await;
  test.assert_ids(vec![1.to_string()]).await;
  test.delete_document(doc_id).await;
  test.assert_ids(vec![]).await;
}

#[tokio::test]
async fn delete_multiple_docs_test() {
  let mut test = CollabPersistenceTest::new(CollabPersistenceConfig::default());
  let db = test.db.clone();

  // Replacing Script variants with function calls
  test
    .create_document_with_collab_db("1".to_string(), db.clone())
    .await;
  test
    .create_document_with_collab_db("2".to_string(), db.clone())
    .await;
  test
    .create_document_with_collab_db("3".to_string(), db.clone())
    .await;
  test.delete_document("1".to_string()).await;
  test.delete_document("2".to_string()).await;
  test.assert_ids(vec![3.to_string()]).await;
}
