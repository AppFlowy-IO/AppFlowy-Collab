use collab_document::blocks::Block;
use collab_persistence::kv_impls::rocks_kv::RocksCollabDB;

use futures::executor::block_on;

use crate::util::{
  document_storage, get_document_data, open_document_with_db, unzip_history_document_db,
  DocumentTest,
};

#[tokio::test]
async fn restore_default_document_test() {
  let uid = 1;
  let doc_id = "1";
  let test = DocumentTest::new(uid, doc_id).await;
  let data1 = test.get_document_data().unwrap();

  let restore_document = open_document_with_db(uid, doc_id, test.db).await;
  let data2 = restore_document.get_document_data().unwrap();

  assert_eq!(data1, data2);
}

#[tokio::test]
async fn restore_default_document_test2() {
  let uid = 1;
  let doc_id = "1";
  let test = DocumentTest::new(uid, doc_id).await;
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

  test.with_transact_mut(|txn| {
    test.insert_block(txn, block.clone(), None).unwrap();
  });

  let restore_document = open_document_with_db(uid, doc_id, test.db).await;
  let restore_block = restore_document.get_block("b1").unwrap();
  assert_eq!(restore_block, block);
}

#[tokio::test]
async fn multiple_thread_create_document_test() {
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

  for i in 0..100 {
    let cloned_db = db.clone();
    let handle = std::thread::spawn(move || {
      let doc = block_on(DocumentTest::new_with_db(
        i,
        &format!("doc_id_{}", i),
        cloned_db,
      ));
      let (page_id, _, _) = get_document_data(&doc.document);
      let block = create_block(page_id, i);
      doc.with_transact_mut(|txn| {
        doc.insert_block(txn, block, None).unwrap();
      });
    });
    handles.push(handle);
  }

  for handle in handles {
    handle.join().unwrap();
  }

  for i in 0..100 {
    let block_id = format!("block_{}", i).to_string();
    let document = open_document_with_db(i, &format!("doc_id_{}", i), db.clone()).await;
    let restore_block = document.get_block(&block_id).unwrap();
    assert_eq!(restore_block.children, format!("children_{}", i));
  }
}

const HISTORY_DOCUMENT_020: &str = "020_document";
#[tokio::test]
async fn open_020_history_document_test() {
  let (_cleaner, db_path) = unzip_history_document_db(HISTORY_DOCUMENT_020).unwrap();
  let db = std::sync::Arc::new(RocksCollabDB::open_opt(db_path, false).unwrap());
  let document = open_document_with_db(
    221439819971039232,
    "631584ec-af71-42c3-94f4-89dcfdafb988",
    db,
  )
  .await
  .get_document_data()
  .unwrap();
  assert_eq!(document.page_id, "Zdu5U1JKpl");
  assert_eq!(document.blocks.len(), 25);
}
