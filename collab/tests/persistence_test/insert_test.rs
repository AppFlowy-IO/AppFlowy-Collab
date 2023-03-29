use crate::script::Script::*;
use crate::script::{disk_plugin, CollabPersistenceTest};

#[test]
fn insert_single_change_and_restore_from_disk() {
  let doc_id = "1".to_string();
  let mut test = CollabPersistenceTest::new();
  test.run_scripts(vec![
    CreateDocumentWithPlugin {
      id: doc_id.clone(),
      plugin: disk_plugin(),
    },
    InsertText {
      id: doc_id.clone(),
      key: "1".to_string(),
      value: "a".into(),
    },
    CloseDocument {
      id: doc_id.to_string(),
    },
    OpenDocumentWithPlugin {
      id: doc_id.to_string(),
    },
    GetText {
      id: doc_id,
      key: "1".to_string(),
      expected: Some("a".into()),
    },
  ]);
}

#[test]
fn insert_multiple_changes_and_restore_from_disk() {
  let mut test = CollabPersistenceTest::new();
  let doc_id = "1".to_string();
  test.run_scripts(vec![
    CreateDocumentWithPlugin {
      id: doc_id.clone(),
      plugin: disk_plugin(),
    },
    InsertText {
      id: doc_id.clone(),
      key: "1".to_string(),
      value: "a".into(),
    },
    InsertText {
      id: doc_id.clone(),
      key: "2".to_string(),
      value: "b".into(),
    },
    InsertText {
      id: doc_id.clone(),
      key: "3".to_string(),
      value: "c".into(),
    },
    InsertText {
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
    OpenDocumentWithPlugin {
      id: doc_id.to_string(),
    },
    GetText {
      id: doc_id.to_string(),
      key: "1".to_string(),
      expected: Some("a".into()),
    },
    GetText {
      id: doc_id.to_string(),
      key: "2".to_string(),
      expected: Some("b".into()),
    },
    GetText {
      id: doc_id.to_string(),
      key: "3".to_string(),
      expected: Some("c".into()),
    },
    GetText {
      id: doc_id,
      key: "4".to_string(),
      expected: Some("d".into()),
    },
  ]);
}

#[test]
fn insert_multiple_docs() {
  let mut test = CollabPersistenceTest::new();
  let disk_plugin = disk_plugin();
  test.run_scripts(vec![
    CreateDocumentWithPlugin {
      id: "1".to_string(),
      plugin: disk_plugin.clone(),
    },
    CreateDocumentWithPlugin {
      id: "2".to_string(),
      plugin: disk_plugin.clone(),
    },
    CreateDocumentWithPlugin {
      id: "3".to_string(),
      plugin: disk_plugin.clone(),
    },
    CreateDocumentWithPlugin {
      id: "4".to_string(),
      plugin: disk_plugin.clone(),
    },
    CreateDocumentWithPlugin {
      id: "5".to_string(),
      plugin: disk_plugin.clone(),
    },
    CreateDocumentWithPlugin {
      id: "6".to_string(),
      plugin: disk_plugin,
    },
    AssertNumOfDocuments { expected: 6 },
  ]);
}
