use crate::script::CollabPersistenceTest;
use crate::script::Script::*;

#[test]
fn insert_single_change_and_restore_from_disk() {
    let doc_id = "1".to_string();
    let mut test = CollabPersistenceTest::new();
    test.run_scripts(vec![
        CreateDocument { id: doc_id.clone() },
        InsertText {
            id: doc_id.clone(),
            key: "1".to_string(),
            value: "a".into(),
        },
        CloseDocument {
            id: doc_id.to_string(),
        },
        OpenDocument {
            id: doc_id.to_string(),
        },
        GetText {
            id: doc_id.clone(),
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
        CreateDocument { id: doc_id.clone() },
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
        OpenDocument {
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
            id: doc_id.to_string(),
            key: "4".to_string(),
            expected: Some("d".into()),
        },
    ]);
}

#[test]
fn insert_multiple_docs() {
    let mut test = CollabPersistenceTest::new();
    test.run_scripts(vec![
        CreateDocument {
            id: "1".to_string(),
        },
        CreateDocument {
            id: "2".to_string(),
        },
        CreateDocument {
            id: "3".to_string(),
        },
        CreateDocument {
            id: "4".to_string(),
        },
        CreateDocument {
            id: "5".to_string(),
        },
        CreateDocument {
            id: "6".to_string(),
        },
        AssertNumOfDocuments { expected: 6 },
    ]);
}
