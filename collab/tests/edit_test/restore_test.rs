#![allow(clippy::all)]

use collab::core::collab::{CollabBuilder, DocStateSource};
use std::collections::HashMap;
use yrs::updates::decoder::Decode;
use yrs::{ReadTxn, Transact, Update};

use crate::util::{setup_log, CollabStateCachePlugin};

#[tokio::test]
async fn restore_from_update() {
  let update_cache = CollabStateCachePlugin::new();
  let collab = CollabBuilder::new(1, "1")
    .with_device_id("1")
    .with_plugin(update_cache.clone())
    .build()
    .unwrap();
  collab.lock().initialize();
  collab.lock().insert("text", "hello world");

  let updates = update_cache.get_doc_state().unwrap();
  let restored_collab = CollabBuilder::new(1, "1")
    .with_device_id("1")
    .with_doc_state(updates)
    .build()
    .unwrap();
  let value = restored_collab.lock().get("text").unwrap();
  let s = value.to_string(&collab.lock().transact());
  assert_eq!(s, "hello world");
}

#[tokio::test]
async fn restore_from_multiple_update() {
  let update_cache = CollabStateCachePlugin::new();
  let collab = CollabBuilder::new(1, "1")
    .with_device_id("1")
    .with_plugin(update_cache.clone())
    .build()
    .unwrap();
  collab.lock().initialize();

  // Insert map
  let mut map = HashMap::new();
  map.insert("1".to_string(), "task 1".to_string());
  map.insert("2".to_string(), "task 2".to_string());
  collab.lock().insert_json_with_path(vec![], "bullet", map);

  let updates = update_cache.get_doc_state().unwrap();
  let restored_collab = CollabBuilder::new(1, "1")
    .with_device_id("1")
    .with_doc_state(updates)
    .build()
    .unwrap();
  assert_eq!(collab.lock().to_json(), restored_collab.lock().to_json());
}

#[tokio::test]
async fn apply_same_update_multiple_time() {
  let update_cache = CollabStateCachePlugin::new();
  let collab = CollabBuilder::new(1, "1")
    .with_device_id("1")
    .with_plugin(update_cache.clone())
    .build()
    .unwrap();
  collab.lock().initialize();
  collab.lock().insert("text", "hello world");

  let updates = update_cache.get_doc_state().unwrap();
  let restored_collab = CollabBuilder::new(1, "1")
    .with_device_id("1")
    .with_doc_state(updates)
    .build()
    .unwrap();

  // It's ok to apply the updates that were already applied
  let doc_state = update_cache.get_doc_state().unwrap();
  restored_collab
    .lock()
    .with_origin_transact_mut(|txn| match doc_state {
      DocStateSource::FromDisk => {
        panic!("doc state should not be empty")
      },
      DocStateSource::FromDocState(doc_state) => {
        txn.apply_update(Update::decode_v1(&doc_state).unwrap());
      },
    });

  assert_json_diff::assert_json_eq!(collab.lock().to_json(), restored_collab.lock().to_json(),);
}

#[tokio::test]
async fn root_change_test() {
  setup_log();
  let collab_1 = CollabBuilder::new(1, "1")
    .with_device_id("1")
    .build()
    .unwrap();
  collab_1.lock().initialize();
  let collab_2 = CollabBuilder::new(1, "1")
    .with_device_id("1")
    .build()
    .unwrap();
  collab_2.lock().initialize();

  {
    let collab_1_guard = collab_1.lock();
    collab_1_guard.with_origin_transact_mut(|txn| {
      collab_1_guard.insert_map_with_txn(txn, "map");
    });
    drop(collab_1_guard);
  }
  {
    let collab_2_guard = collab_2.lock();
    collab_2_guard.with_origin_transact_mut(|txn| {
      collab_2_guard.insert_map_with_txn(txn, "map");
    });
    drop(collab_2_guard);
  }

  let map_2 = {
    let collab_guard = collab_2.lock();
    let txn = collab_guard.transact();
    let map_2 = collab_guard.get_map_with_txn(&txn, vec!["map"]).unwrap();
    drop(txn);

    collab_guard.with_origin_transact_mut(|txn| {
      map_2.insert_with_txn(txn, "1", "a");
      map_2.insert_with_txn(txn, "2", "b");
    });
    map_2
  };

  let sv_1 = collab_1.lock().get_doc().transact().state_vector();
  let sv_1_update = collab_2
    .lock()
    .get_doc()
    .transact()
    .encode_state_as_update_v1(&sv_1);

  let map_1 = {
    let collab_1_guard = collab_1.lock();
    collab_1_guard.with_origin_transact_mut(|txn| {
      let update = Update::decode_v1(&sv_1_update).unwrap();
      txn.apply_update(update);
    });

    let txn = collab_1_guard.transact();
    collab_1_guard.get_map_with_txn(&txn, vec!["map"]).unwrap()
  };

  let a = map_1.to_json_value().unwrap();
  let b = map_2.to_json_value().unwrap();

  println!("a: {}", a);
  println!("b: {}", b);
  // assert_eq!(a, b);
}
