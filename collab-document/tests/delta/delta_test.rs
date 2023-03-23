use crate::util::create_document;

#[test]
fn create_document_test() {
    let doc_id = "1";
    let test = create_document(doc_id);
    test.document.attrs().insert_with_key("1", |builder| {
        builder.with_type("text").with_data("hello world").build()
    });

    let attrs = test.document.attrs();
    let txn = attrs.transact();
    let ty = attrs
        .get_map_with_txn(&txn, "1")
        .unwrap()
        .get_str_with_txn(&txn, "type")
        .unwrap();

    assert_eq!(ty, "text");
}
