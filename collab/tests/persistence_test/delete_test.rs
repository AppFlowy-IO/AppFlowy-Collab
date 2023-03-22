use crate::script::CollabPersistenceTest;
use crate::script::Script::*;

#[test]
fn delete_single_doc_test() {
    let mut test = CollabPersistenceTest::new();
    test.run_scripts(vec![
        CreateDocument {
            id: "1".to_string(),
        },
        AssertNumOfDocuments { expected: 1 },
        DeleteDocument {
            id: "1".to_string(),
        },
        AssertNumOfDocuments { expected: 0 },
    ]);
}
#[test]
fn delete_multiple_docs_test() {
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
        DeleteDocument {
            id: "1".to_string(),
        },
        DeleteDocument {
            id: "2".to_string(),
        },
        AssertNumOfDocuments { expected: 1 },
    ]);
}
