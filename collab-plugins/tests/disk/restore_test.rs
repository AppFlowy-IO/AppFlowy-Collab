use crate::disk::util::rocks_db;
use collab_plugins::CollabKVDB;
use collab_plugins::local_storage::kv::doc::{
  CollabKVAction, extract_object_id_from_key_v1, migrate_old_keys,
};
use collab_plugins::local_storage::kv::keys::{make_doc_id_key_v0, make_doc_id_key_v1};
use collab_plugins::local_storage::kv::{KVStore, KVTransactionDB};
use std::thread;
use uuid::Uuid;
use yrs::{Doc, GetString, Text, Transact};

#[tokio::test]
async fn single_thread_test() {
  let workspace_id = Uuid::new_v4().to_string();
  let (path, db) = rocks_db();
  for i in 0..100 {
    let oid = format!("doc_{}", i);
    let doc = Doc::new();
    {
      let txn = doc.transact();
      db.with_write_txn(|db_w_txn| {
        db_w_txn
          .create_new_doc(1, &workspace_id, &oid, &txn)
          .unwrap();
        Ok(())
      })
      .unwrap();
    }
    {
      let text = doc.get_or_insert_text("text");
      let mut txn = doc.transact_mut();
      text.insert(&mut txn, 0, &format!("Hello, world! {}", i));
      let update = txn.encode_update_v1();
      db.with_write_txn(|w| {
        w.push_update(1, &workspace_id, &oid, &update).unwrap();
        Ok(())
      })
      .unwrap();
    }
  }
  drop(db);

  let db = CollabKVDB::open(path).unwrap();
  for i in 0..100 {
    let oid = format!("doc_{}", i);
    let doc = Doc::new();
    {
      let mut txn = doc.transact_mut();
      db.read_txn()
        .load_doc_with_txn(1, &workspace_id, &oid, &mut txn)
        .unwrap();
    }
    let text = doc.get_or_insert_text("text");
    let txn = doc.transact();
    assert_eq!(text.get_string(&txn), format!("Hello, world! {}", i));
  }
}

#[tokio::test]
async fn rocks_multiple_thread_test() {
  let (path, db) = rocks_db();
  let mut handles = vec![];
  let workspace_id = Uuid::new_v4().to_string();
  for i in 0..100 {
    let cloned_workspace_id = workspace_id.clone();
    let cloned_db = db.clone();
    let handle = thread::spawn(move || {
      let oid = format!("doc_{}", i);
      let doc = Doc::new();
      {
        let txn = doc.transact();
        cloned_db
          .with_write_txn(|store| store.create_new_doc(1, &cloned_workspace_id, &oid, &txn))
          .unwrap();
      }
      {
        let text = doc.get_or_insert_text("text");
        let mut txn = doc.transact_mut();
        text.insert(&mut txn, 0, &format!("Hello, world! {}", i));
        let update = txn.encode_update_v1();
        cloned_db
          .with_write_txn(|store| store.push_update(1, &cloned_workspace_id, &oid, &update))
          .unwrap();
      }
    });
    handles.push(handle);
  }

  for handle in handles {
    handle.join().unwrap();
  }
  drop(db);

  let db = CollabKVDB::open(path).unwrap();
  for i in 0..100 {
    let oid = format!("doc_{}", i);
    let doc = Doc::new();
    {
      let mut txn = doc.transact_mut();
      db.read_txn()
        .load_doc_with_txn(1, &workspace_id, &oid, &mut txn)
        .unwrap();
    }
    let text = doc.get_or_insert_text("text");
    let txn = doc.transact();
    assert_eq!(text.get_string(&txn), format!("Hello, world! {}", i));
  }
}
#[tokio::test]
async fn multiple_workspace_test() {
  let (_, db) = rocks_db();

  // Define multiple workspaces with different numbers of objects and user IDs
  let workspaces = [
    ("Workspace 1", 2), // Workspace 1 with 2 objects
    ("Workspace 2", 3), // Workspace 2 with 3 objects
    ("Workspace 3", 1),
  ];

  for (index, (workspace_name, num_objects)) in workspaces.iter().enumerate() {
    let workspace_id = Uuid::new_v4().to_string();
    let user_id = index as i64 + 1; // Assign a unique user ID (1, 2, 3, ...)

    for _ in 0..*num_objects {
      let object_id = Uuid::new_v4().to_string();
      let doc = Doc::new();
      // Create a new document in the workspace with the current user ID
      {
        let txn = doc.transact();
        db.with_write_txn(|store| store.create_new_doc(user_id, &workspace_id, &object_id, &txn))
          .unwrap();
      }

      // Insert content into the document
      {
        let text = doc.get_or_insert_text("text");
        let mut txn = doc.transact_mut();
        let content = format!("Content for {} in {}", object_id, workspace_name);
        text.insert(&mut txn, 0, &content);
        let update = txn.encode_update_v1();
        db.with_write_txn(|store| store.push_update(user_id, &workspace_id, &object_id, &update))
          .unwrap();
      }
    }

    // Test get_all_docs_for_user for the current user and workspace
    let docs_iter = db
      .read_txn()
      .get_all_object_ids(user_id, &workspace_id)
      .unwrap()
      .collect::<Vec<String>>();
    let doc_count: usize = docs_iter.len();
    assert_eq!(
      doc_count, *num_objects,
      "Unexpected document count for user {} in {}",
      user_id, workspace_name
    );
  }

  // Count the total number of workspaces
  let workspace_count = db.read_txn().get_all_workspace_ids().unwrap().len();
  assert_eq!(workspace_count, workspaces.len());
}

#[tokio::test]
async fn test_migrate_old_keys() {
  let workspace_id = Uuid::new_v4().to_string();
  let workspace_id_bytes = workspace_id.as_bytes();
  let (_, db) = rocks_db();

  // Insert old keys into the database
  let num_docs = 5;
  let uid: i64 = 123;
  let uid_id_bytes = &uid.to_be_bytes();
  let mut object_ids = Vec::new();

  for _ in 0..num_docs {
    let object_id = Uuid::new_v4().to_string();
    object_ids.push(object_id.clone());

    let old_key = make_doc_id_key_v0(uid_id_bytes, object_id.as_ref());
    let value = object_id.clone().into_bytes(); // Use object_id as value

    db.with_write_txn(|db_w_txn| {
      db_w_txn.insert(old_key.clone(), value.clone())?;
      Ok(())
    })
    .unwrap();
  }

  // Perform migration of old keys
  {
    let w = db.write_txn();
    migrate_old_keys(&w, &workspace_id).unwrap();
    w.commit_transaction().unwrap();
  }

  // Verify that old keys were migrated to new keys
  for object_id in object_ids {
    let new_key = make_doc_id_key_v1(uid_id_bytes, workspace_id.as_bytes(), object_id.as_bytes());

    let oid = extract_object_id_from_key_v1(&new_key, uid_id_bytes.len(), workspace_id_bytes.len())
      .map(|v| String::from_utf8(v.to_vec()).unwrap())
      .unwrap();
    assert_eq!(oid, object_id);
  }
}
