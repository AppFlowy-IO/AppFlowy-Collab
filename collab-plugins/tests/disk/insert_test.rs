use crate::disk::script::Script::*;
use crate::disk::script::{disk_plugin, CollabPersistenceTest};
use collab_plugins::local_storage::CollabPersistenceConfig;

#[tokio::test]
async fn insert_single_change_and_restore_from_disk() {
  let doc_id = "1".to_string();
  let mut test = CollabPersistenceTest::new(CollabPersistenceConfig::new());
  let (_db, disk_plugin) = disk_plugin(test.uid);
  test
    .run_scripts(vec![
      CreateDocumentWithDiskPlugin {
        id: doc_id.clone(),
        plugin: disk_plugin,
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
  let (_db, disk_plugin) = disk_plugin(test.uid);
  test
    .run_scripts(vec![
      CreateDocumentWithDiskPlugin {
        id: doc_id.clone(),
        plugin: disk_plugin,
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
  let (_db, disk_plugin) = disk_plugin(test.uid);
  test
    .run_scripts(vec![
      CreateDocumentWithDiskPlugin {
        id: "1".to_string(),
        plugin: disk_plugin.clone(),
      },
      CreateDocumentWithDiskPlugin {
        id: "2".to_string(),
        plugin: disk_plugin.clone(),
      },
      CreateDocumentWithDiskPlugin {
        id: "3".to_string(),
        plugin: disk_plugin.clone(),
      },
      CreateDocumentWithDiskPlugin {
        id: "4".to_string(),
        plugin: disk_plugin.clone(),
      },
      CreateDocumentWithDiskPlugin {
        id: "5".to_string(),
        plugin: disk_plugin.clone(),
      },
      CreateDocumentWithDiskPlugin {
        id: "6".to_string(),
        plugin: disk_plugin,
      },
      AssertNumOfDocuments { expected: 6 },
    ])
    .await;
}
