use crate::disk::script::Script::*;
use crate::disk::script::{disk_plugin, CollabPersistenceTest};
use collab_plugins::local_storage::CollabPersistenceConfig;

#[tokio::test]
async fn delete_single_doc_test() {
  let mut test = CollabPersistenceTest::new(CollabPersistenceConfig::default());
  let doc_id = "1".to_string();
  let (_db, disk_plugin) = disk_plugin(test.uid);
  test
    .run_scripts(vec![
      CreateDocumentWithDiskPlugin {
        id: doc_id.clone(),
        plugin: disk_plugin,
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
        plugin: disk_plugin,
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
