use crate::disk::script::CollabPersistenceTest;
use crate::disk::script::Script::*;
use collab_plugins::local_storage::CollabPersistenceConfig;

#[tokio::test]
async fn delete_single_doc_test() {
  let mut test = CollabPersistenceTest::new(CollabPersistenceConfig::default());
  let doc_id = "1".to_string();
  test
    .run_scripts(vec![
      CreateDocumentWithCollabDB {
        id: doc_id.clone(),
        db: test.db.clone(),
      },
      AssertNumOfDocuments { expected: 1 },
      DeleteDocument { id: doc_id },
      AssertNumOfDocuments { expected: 0 },
    ])
    .await;
}
#[tokio::test]
async fn delete_multiple_docs_test() {
  let mut test = CollabPersistenceTest::new(CollabPersistenceConfig::default());
  let db = test.db.clone();
  test
    .run_scripts(vec![
      CreateDocumentWithCollabDB {
        id: "1".to_string(),
        db: db.clone(),
      },
      CreateDocumentWithCollabDB {
        id: "2".to_string(),
        db: db.clone(),
      },
      CreateDocumentWithCollabDB {
        id: "3".to_string(),
        db: db.clone(),
      },
      DeleteDocument {
        id: "1".to_string(),
      },
      DeleteDocument {
        id: "2".to_string(),
      },
      AssertNumOfDocuments { expected: 1 },
    ])
    .await;
}
