use std::thread;

use collab_persistence::doc::YrsDocAction;
use collab_persistence::kv::rocks_kv::RocksCollabDB;
use collab_persistence::kv::sled_lv::SledCollabDB;
use collab_persistence::kv::KVStore;
use yrs::{Doc, GetString, Text, Transact};

use crate::util::{rocks_db, sled_db};

#[test]
fn single_thread_test() {
  let (path, db) = sled_db();
  for i in 0..100 {
    let oid = format!("doc_{}", i);
    let doc = Doc::new();
    {
      let txn = doc.transact();
      let store = db.kv_store_impl();
      store.create_new_doc(1, &oid, &txn).unwrap();
    }
    {
      let text = doc.get_or_insert_text("text");
      let mut txn = doc.transact_mut();
      text.insert(&mut txn, 0, &format!("Hello, world! {}", i));
      let update = txn.encode_update_v1();
      db.kv_store_impl().push_update(1, &oid, &update).unwrap();
    }
  }
  drop(db);

  let db = SledCollabDB::open(path).unwrap();
  for i in 0..100 {
    let oid = format!("doc_{}", i);
    let doc = Doc::new();
    {
      let mut txn = doc.transact_mut();
      db.kv_store_impl().load_doc(1, &oid, &mut txn).unwrap();
    }
    let text = doc.get_or_insert_text("text");
    let txn = doc.transact();
    assert_eq!(text.get_string(&txn), format!("Hello, world! {}", i));
  }
}

#[test]
fn sled_multiple_thread_test() {
  let (path, db) = sled_db();
  let mut handles = vec![];
  for i in 0..100 {
    let cloned_db = db.clone();
    let handle = thread::spawn(move || {
      let oid = format!("doc_{}", i);
      let doc = Doc::new();
      {
        let txn = doc.transact();
        let store = cloned_db.kv_store_impl();
        store.create_new_doc(1, &oid, &txn).unwrap();
        store.commit().unwrap();
      }
      {
        let text = doc.get_or_insert_text("text");
        let mut txn = doc.transact_mut();
        text.insert(&mut txn, 0, &format!("Hello, world! {}", i));
        let update = txn.encode_update_v1();
        let store = cloned_db.kv_store_impl();
        store.push_update(1, &oid, &update).unwrap();
        store.commit().unwrap();
      }
    });
    handles.push(handle);
  }

  for handle in handles {
    handle.join().unwrap();
  }
  drop(db);

  let db = SledCollabDB::open(path).unwrap();
  for i in 0..100 {
    let oid = format!("doc_{}", i);
    let doc = Doc::new();
    {
      let mut txn = doc.transact_mut();
      db.kv_store_impl().load_doc(1, &oid, &mut txn).unwrap();
    }
    let text = doc.get_or_insert_text("text");
    let txn = doc.transact();
    assert_eq!(text.get_string(&txn), format!("Hello, world! {}", i));
  }
}

#[test]
fn rocks_multiple_thread_test() {
  let (path, db) = rocks_db();
  let mut handles = vec![];
  for i in 0..100 {
    let cloned_db = db.clone();
    let handle = thread::spawn(move || {
      let oid = format!("doc_{}", i);
      let doc = Doc::new();
      {
        let txn = doc.transact();
        let store = cloned_db.kv_store_impl();
        store.create_new_doc(1, &oid, &txn).unwrap();
        store.commit().unwrap();
      }
      {
        let text = doc.get_or_insert_text("text");
        let mut txn = doc.transact_mut();
        text.insert(&mut txn, 0, &format!("Hello, world! {}", i));
        let update = txn.encode_update_v1();
        let store = cloned_db.kv_store_impl();
        store.push_update(1, &oid, &update).unwrap();
        store.commit().unwrap();
      }
    });
    handles.push(handle);
  }

  for handle in handles {
    handle.join().unwrap();
  }
  drop(db);

  let db = RocksCollabDB::open(path).unwrap();
  for i in 0..100 {
    let oid = format!("doc_{}", i);
    let doc = Doc::new();
    {
      let mut txn = doc.transact_mut();
      db.kv_store_impl().load_doc(1, &oid, &mut txn).unwrap();
    }
    let text = doc.get_or_insert_text("text");
    let txn = doc.transact();
    assert_eq!(text.get_string(&txn), format!("Hello, world! {}", i));
  }
}
