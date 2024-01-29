use collab_document::{
  blocks::{Block, BlockAction, BlockActionPayload, BlockActionType},
  document::DocumentIndexContent,
};
use nanoid::nanoid;

use crate::util::{
  apply_actions, get_document_data, insert_block, open_document_with_db, DocumentTest,
};

#[tokio::test]
async fn insert_block_with_empty_parent_id_and_empty_prev_id() {
  let uid = 1;
  let doc_id = "1";
  let test = DocumentTest::new(uid, doc_id).await;
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
      prev_id: None,
      parent_id: Some(page_id),
      block: Some(block),
      delta: None,
      text_id: None,
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

#[tokio::test]
#[should_panic]
async fn open_empty_document() {
  let doc_id = "1";

  // the document is not exist, so this should panic
  let document_test = DocumentTest::new(1, doc_id).await;
  let document = document_test.document;
  let data = document.get_document_data();
  assert!(data.is_err());
}

#[tokio::test]
async fn reopen_document() {
  let doc_id = "1";
  let test = DocumentTest::new(1, doc_id).await;
  let document = test.document;
  let (page_id, _, _) = get_document_data(&document);

  // close document
  drop(document);

  let document = open_document_with_db(1, doc_id, test.db).await;
  let (page_id2, _, _) = get_document_data(&document);
  assert_eq!(page_id, page_id2);
}

#[tokio::test]
async fn document_index_data_from_document() {
  let doc_id = "1";
  let test = DocumentTest::new(1, doc_id).await;
  let document = test.document;

  let (page_id, _blocks, _children_map) = get_document_data(&document);
  let index_content = DocumentIndexContent::from(&document);
  assert_eq!(index_content.page_id, page_id);
  assert_eq!(index_content.text, "");

  let block_id = nanoid!(10);
  let text_id = nanoid!(10);
  let block = Block {
    id: block_id,
    ty: "paragraph".to_owned(),
    parent: page_id.clone(),
    children: "".to_string(),
    external_id: Some(text_id.clone()),
    external_type: Some("text".to_owned()),
    data: Default::default(),
  };

  insert_block(&document, block, "".to_string()).unwrap();
  document.create_text(
    &text_id,
    r#"[{"insert": "Hello "}, {"insert": "world!"}]"#.to_owned(),
  );

  let index_content = DocumentIndexContent::from(&document);
  assert_eq!(index_content.page_id, page_id);
  assert_eq!(index_content.text, "Hello world!");
}
