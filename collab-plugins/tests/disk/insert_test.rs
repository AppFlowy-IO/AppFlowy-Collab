use crate::disk::script::CollabPersistenceTest;
use crate::disk::script::Script::*;
use collab_plugins::local_storage::CollabPersistenceConfig;

#[tokio::test]
async fn insert_single_change_and_restore_from_disk() {
  let doc_id = "1".to_string();
  let mut test = CollabPersistenceTest::new(CollabPersistenceConfig::new());
  let db = test.db.clone();
  test
    .run_scripts(vec![
      CreateDocumentWithCollabDB {
        id: doc_id.clone(),
        db: db.clone(),
      },
      InsertKeyValue {
        id: doc_id.clone(),
        key: "1".to_string(),
        value: "a".into(),
      },
      CloseDocument {
        id: doc_id.to_string(),
      },
      OpenDocumentWithDiskPlugin {
        id: doc_id.to_string(),
      },
      GetValue {
        id: doc_id,
        key: "1".to_string(),
        expected: Some("a".into()),
      },
    ])
    .await;
}

#[tokio::test]
async fn insert_multiple_changes_and_restore_from_disk() {
  let mut test = CollabPersistenceTest::new(CollabPersistenceConfig::new());
  let doc_id = "1".to_string();
  let db = test.db.clone();
  test
    .run_scripts(vec![
      CreateDocumentWithCollabDB {
        id: doc_id.clone(),
        db: db.clone(),
      },
      InsertKeyValue {
        id: doc_id.clone(),
        key: "1".to_string(),
        value: "a".into(),
      },
      InsertKeyValue {
        id: doc_id.clone(),
        key: "2".to_string(),
        value: "b".into(),
      },
      InsertKeyValue {
        id: doc_id.clone(),
        key: "3".to_string(),
        value: "c".into(),
      },
      InsertKeyValue {
        id: doc_id.clone(),
        key: "4".to_string(),
        value: "d".into(),
      },
      AssertNumOfUpdates {
        id: doc_id.clone(),
        expected: 4,
      },
      CloseDocument {
        id: doc_id.to_string(),
      },
      OpenDocumentWithDiskPlugin {
        id: doc_id.to_string(),
      },
      GetValue {
        id: doc_id.to_string(),
        key: "1".to_string(),
        expected: Some("a".into()),
      },
      GetValue {
        id: doc_id.to_string(),
        key: "2".to_string(),
        expected: Some("b".into()),
      },
      GetValue {
        id: doc_id.to_string(),
        key: "3".to_string(),
        expected: Some("c".into()),
      },
      GetValue {
        id: doc_id,
        key: "4".to_string(),
        expected: Some("d".into()),
      },
    ])
    .await;
}

#[tokio::test]
async fn insert_multiple_docs() {
  let mut test = CollabPersistenceTest::new(CollabPersistenceConfig::new());
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
      CreateDocumentWithCollabDB {
        id: "4".to_string(),
        db: db.clone(),
      },
      CreateDocumentWithCollabDB {
        id: "5".to_string(),
        db: db.clone(),
      },
      CreateDocumentWithCollabDB {
        id: "6".to_string(),
        db: db.clone(),
      },
      AssertNumOfDocuments { expected: 6 },
    ])
    .await;
}
