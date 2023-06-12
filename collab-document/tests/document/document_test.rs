use collab_document::blocks::{Block, BlockAction, BlockActionPayload, BlockActionType};
use nanoid::nanoid;

use crate::util::{
  apply_actions, create_document, create_document_with_db, db, get_document_data,
  open_document_with_db,
};

#[test]
fn insert_block_with_empty_parent_id_and_empty_prev_id() {
  let uid = 1;
  let doc_id = "1";
  let test = create_document(uid, doc_id);
  let (page_id, _, _) = get_document_data(&test.document);
  let block_id = nanoid!(10);
  let block = Block {
    id: block_id.clone(),
    ty: "".to_string(),
    parent: "".to_string(), // empty parent id
    children: "".to_string(),
    external_id: None,
    external_type: None,
    data: Default::default(),
  };
  let insert_action = BlockAction {
    action: BlockActionType::Insert,
    payload: BlockActionPayload {
      block,
      prev_id: None,
      parent_id: Some(page_id),
    },
  };
  apply_actions(&test.document, vec![insert_action]);
  let (page_id, blocks, meta) = get_document_data(&test.document);

  // the block's parent_id should be the page_id
  let block = blocks.get(&block_id).unwrap();
  assert_eq!(block.parent, page_id);

  // the page's children should contain the block_id
  let page_children_id = test.document.get_block(&page_id).unwrap().children;
  let page_children = meta.get(&page_children_id).unwrap().to_vec();
  assert!(page_children.contains(&block_id));
}

#[test]
fn open_empty_document() {
  let doc_id = "1";
  let db = db();
  let document_test = open_document_with_db(1, doc_id, db);
  let document = document_test.document;
  let data = document.get_document();
  assert!(data.is_err());
}

#[test]
fn reopen_document() {
  let doc_id = "1";
  let db = db();
  let test = create_document_with_db(1, doc_id, db.clone());
  let document = test.document;
  let (page_id, _, _) = get_document_data(&document);

  // close document
  drop(document);

  let test = open_document_with_db(1, doc_id, db);
  let document = test.document;
  let (page_id2, _, _) = get_document_data(&document);
  assert_eq!(page_id, page_id2);
}
