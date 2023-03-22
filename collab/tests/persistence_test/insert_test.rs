use crate::script::CollabPersistenceTest;
use crate::script::Script::*;

#[test]
fn insert_single_change_and_restore_from_disk() {
    let test = CollabPersistenceTest::new();
    test.run_scripts(vec![InsertText {
        key: "1".to_string(),
        value: "a".into(),
    }]);
    let db_path = test.db_path.clone();
    let cid = test.cid.clone();
    drop(test);

    // Load from disk
    let test = CollabPersistenceTest::new_with_path(db_path, cid);
    test.run_scripts(vec![GetText {
        key: "1".to_string(),
        expected: Some("a".into()),
    }])
}

#[test]
fn insert_multiple_changes_and_restore_from_disk() {
    let test = CollabPersistenceTest::new();
    test.run_scripts(vec![
        InsertText {
            key: "1".to_string(),
            value: "a".into(),
        },
        InsertText {
            key: "2".to_string(),
            value: "b".into(),
        },
        InsertText {
            key: "3".to_string(),
            value: "c".into(),
        },
        InsertText {
            key: "4".to_string(),
            value: "d".into(),
        },
        AssertNumOfUpdates { expected: 4 },
    ]);

    let db_path = test.db_path.clone();
    let cid = test.cid.clone();
    drop(test);

    // Load from disk
    let test = CollabPersistenceTest::new_with_path(db_path, cid);
    test.run_scripts(vec![
        GetText {
            key: "1".to_string(),
            expected: Some("a".into()),
        },
        GetText {
            key: "2".to_string(),
            expected: Some("b".into()),
        },
        GetText {
            key: "3".to_string(),
            expected: Some("c".into()),
        },
        GetText {
            key: "4".to_string(),
            expected: Some("d".into()),
        },
    ])
}
