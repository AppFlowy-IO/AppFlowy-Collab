use crate::util::create_document;
use collab_document::blocks::{TextAction, TextData};

#[test]
fn create_block_test() {
    let doc_id = "1";
    let test = create_document(doc_id);
    // Create block
    test.document.blocks.create_block("1", |builder| {
        builder.with_type("text").with_data("hello world").build()
    });

    // Update block
    let blocks = &test.document.blocks;
    let mut block_map = blocks.get_block("1").unwrap();
    blocks.with_transact_mut(|txn| {
        block_map.set_data(txn, "hello appflowy".to_string());
    });

    // Get block
    let txn = blocks.transact();
    let block = block_map.into_object(&txn);

    assert_eq!(block.ty, "text");
    assert_eq!(block.data, "hello appflowy");
}

#[test]
fn update_block_test() {
    let doc_id = "1";
    let test = create_document(doc_id);
    // Create block
    test.document.blocks.create_block("1", |builder| {
        builder.with_type("text").with_data("hello world").build()
    });

    // Update block
    let blocks = &test.document.blocks;
    let mut map_ref = blocks.get_block("1").unwrap();
    blocks.with_transact_mut(|txn| {
        map_ref.set_data(txn, "hello appflowy".to_string());
    });

    // Get block
    let block = map_ref.into_object(&blocks.transact());

    assert_eq!(block.ty, "text");
    assert_eq!(block.data, "hello appflowy");
}
