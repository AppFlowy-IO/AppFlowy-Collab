use collab_document::blocks::Block;
use uuid::Uuid;

use collab_plugins::CollabKVDB;

use crate::util::{
  DocumentTest, document_storage, get_document_data, open_document_with_db,
  unzip_history_document_db,
};

#[test]
fn restore_default_document_test() {
  let uid = 1;
  let doc_id = "1";
  let test = DocumentTest::new(uid, doc_id);
  let data1 = test.get_document_data().unwrap();

  let restore_document = open_document_with_db(uid, &test.workspace_id, doc_id, test.db);
  let data2 = restore_document.get_document_data().unwrap();

  assert_eq!(data1, data2);
}

#[test]
fn restore_default_document_test2() {
  let uid = 1;
  let doc_id = "1";
  let mut test = DocumentTest::new(uid, doc_id);
  let (page_id, _, _) = get_document_data(&test.document);
  let block = Block {
    id: "b1".to_string(),
    ty: "".to_string(),
    parent: page_id,
    children: "children".to_string(),
    external_id: None,
    external_type: None,
    data: Default::default(),
  };

  test.document.insert_block(block.clone(), None).unwrap();

  let restore_document = open_document_with_db(uid, &test.workspace_id, doc_id, test.db);
  let restore_block = restore_document.get_block("b1").unwrap();
  assert_eq!(restore_block, block);
}

#[test]
fn multiple_thread_create_document_test() {
  let db = document_storage();
  let mut handles = vec![];

  let create_block = |page_id: String, index: i64| Block {
    id: format!("block_{}", index),
    ty: "".to_string(),
    parent: page_id,
    children: format!("children_{}", index),
    external_id: None,
    external_type: None,
    data: Default::default(),
  };

  let workspace_id = Uuid::new_v4().to_string();

  for i in 0..100 {
    let cloned_workspace_id = workspace_id.clone();
    let cloned_db = db.clone();
    let handle = std::thread::spawn(move || {
      let mut doc = DocumentTest::new_with_db(
        i,
        cloned_workspace_id.clone(),
        &format!("doc_id_{}", i),
        cloned_db,
      );
      let (page_id, _, _) = get_document_data(&doc.document);
      let block = create_block(page_id, i);
      doc.document.insert_block(block, None).unwrap();
    });
    handles.push(handle);
  }

  for handle in handles {
    handle.join().unwrap();
  }

  for i in 0..100 {
    let block_id = format!("block_{}", i).to_string();
    let document = open_document_with_db(i, &workspace_id, &format!("doc_id_{}", i), db.clone());
    let restore_block = document.get_block(&block_id).unwrap();
    assert_eq!(restore_block.children, format!("children_{}", i));
  }
}

const HISTORY_DOCUMENT_020: &str = "020_document";
#[test]
fn open_020_history_document_test() {
  let workspace_id = Uuid::new_v4().to_string();
  let (_cleaner, db_path) = unzip_history_document_db(HISTORY_DOCUMENT_020).unwrap();
  let db = std::sync::Arc::new(CollabKVDB::open(db_path).unwrap());
  let document = open_document_with_db(
    221439819971039232,
    &workspace_id,
    "631584ec-af71-42c3-94f4-89dcfdafb988",
    db,
  )
  .get_document_data()
  .unwrap();
  assert_eq!(document.page_id, "Zdu5U1JKpl");
  assert_eq!(document.blocks.len(), 25);
}
