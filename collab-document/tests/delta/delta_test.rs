use crate::util::create_document;

#[test]
fn create_document_test() {
    let doc_id = "123";
    let test = create_document(doc_id);
    let s = test.document.to_json().unwrap();
    assert_eq!(
        s,
        r#"{"attributes":{"0Q4Q":{"type":"text","data":"hello world"}}}"#
    );
}
