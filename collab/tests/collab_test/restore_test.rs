use std::collections::HashMap;

use collab::core::collab::CollabBuilder;
use collab::preclude::MapRefExtension;
use serde_json::json;
use yrs::types::ToJson;
use yrs::updates::decoder::Decode;
use yrs::{Doc, Map, ReadTxn, Transact, Update};

use crate::helper::{setup_log, CollabStateCachePlugin};

#[test]
fn restore_from_update() {
  let update_cache = CollabStateCachePlugin::new();
  let collab = CollabBuilder::new(1, "1")
    .with_plugin(update_cache.clone())
    .build();
  collab.lock().initialize();
  collab.lock().insert("text", "hello world");

  let updates = update_cache.get_updates().unwrap();
  let restored_collab = CollabBuilder::new(1, "1").build_with_updates(updates);
  let value = restored_collab.lock().get("text").unwrap();
  let s = value.to_string(&collab.lock().transact());
  assert_eq!(s, "hello world");
}

#[test]
fn restore_from_multiple_update() {
  let update_cache = CollabStateCachePlugin::new();
  let collab = CollabBuilder::new(1, "1")
    .with_plugin(update_cache.clone())
    .build();
  collab.initial();

  // Insert map
  let mut map = HashMap::new();
  map.insert("1".to_string(), "task 1".to_string());
  map.insert("2".to_string(), "task 2".to_string());
  collab.lock().insert_json_with_path(vec![], "bullet", map);

  let updates = update_cache.get_updates().unwrap();
  let restored_collab = CollabBuilder::new(1, "1").build_with_updates(updates);
  assert_eq!(collab.lock().to_json(), restored_collab.lock().to_json());
}

#[test]
fn apply_same_update_multiple_time() {
  let update_cache = CollabStateCachePlugin::new();
  let collab = CollabBuilder::new(1, "1")
    .with_plugin(update_cache.clone())
    .build();
  collab.initial();
  collab.lock().insert("text", "hello world");

  let updates = update_cache.get_updates().unwrap();
  let restored_collab = CollabBuilder::new(1, "1").build_with_updates(updates);

  // It's ok to apply the updates that were already applied
  let updates = update_cache.get_updates().unwrap();
  restored_collab.lock().with_transact_mut(|txn| {
    for update in updates {
      txn.apply_update(update);
    }
  });

  assert_json_diff::assert_json_eq!(collab.lock().to_json(), restored_collab.lock().to_json(),);
}

#[test]
fn apply_unordered_updates() {
  let update_cache = CollabStateCachePlugin::new();
  let collab = CollabBuilder::new(1, "1")
    .with_plugin(update_cache.clone())
    .build();
  collab.lock().initialize();
  collab.lock().insert("text", "hello world");

  // Insert map
  let mut map = HashMap::new();
  map.insert("1".to_string(), "task 1".to_string());
  map.insert("2".to_string(), "task 2".to_string());
  collab.lock().insert("bullet", map);

  let mut updates = update_cache.get_updates().unwrap();
  updates.reverse();

  let restored_collab = CollabBuilder::new(1, "1").build();
  restored_collab.lock().initialize();
  restored_collab.lock().with_transact_mut(|txn| {
    //Out of order updates from the same peer will be stashed internally and their
    // integration will be postponed until missing blocks arrive first.
    for update in updates {
      txn.apply_update(update);
    }
  });

  assert_json_diff::assert_json_eq!(
    json!( {
      "text": "hello world"
    }),
    restored_collab.to_json_value()
  );
}

#[test]
fn root_change_test() {
  setup_log();
  let collab_1 = CollabBuilder::new(1, "1").build();
  collab_1.lock().initialize();
  let collab_2 = CollabBuilder::new(1, "1").build();
  collab_2.lock().initialize();

  {
    let collab_guard = collab_1.lock();
    collab_guard.with_transact_mut(|txn| {
      collab_guard.create_map_with_txn(txn, "map");
    });
    drop(collab_guard);

    let collab_2_guard = collab_2.lock();
    collab_2_guard.with_transact_mut(|txn| {
      collab_2_guard.create_map_with_txn(txn, "map");
    });
  }

  let map_2 = {
    let collab_guard = collab_2.lock();
    let txn = collab_guard.transact();
    collab_guard.get_map_with_txn(&txn, vec!["map"]).unwrap()
  };

  {
    collab_2.lock().with_transact_mut(|txn| {
      map_2.insert_with_txn(txn, "1", "a");
      map_2.insert_with_txn(txn, "2", "b");
    });
  }

  let map_1 = {
    let collab_guard = collab_1.lock();
    let txn = collab_guard.transact();
    collab_guard.get_map_with_txn(&txn, vec!["map"]).unwrap()
  };

  let sv_1 = collab_1.lock().get_doc().transact().state_vector();
  let sv_update = collab_2
    .lock()
    .get_doc()
    .transact()
    .encode_state_as_update_v1(&sv_1);
  {
    let collab_guard = collab_1.lock();
    collab_guard.with_transact_mut(|txn| {
      let update = Update::decode_v1(&sv_update).unwrap();
      txn.apply_update(update);
    });
  }

  let map_3 = {
    let collab_guard = collab_1.lock();
    let txn = collab_guard.transact();
    let map = collab_guard.get_map_with_txn(&txn, vec!["map"]).unwrap();
    drop(txn);
    drop(collab_guard);
    map
  };

  let a = map_1.to_json_value();
  let b = map_2.to_json_value();
  let c = map_3.to_json_value();

  println!("a: {}", a);
  println!("b: {}", b);
  println!("c: {}", c);
}

// #[test]
// fn root_change_test2() {
//   let collab_1 = Doc::new();
//   let collab_2 = Doc::new();
//
//   let map_1 = collab_1.get_or_insert_map("map");
//   let map_2 = collab_2.get_or_insert_map("map");
//
//   {
//     let mut txn = collab_2.transact_mut();
//     map_2.insert(&mut txn, "1", "a");
//     map_2.insert(&mut txn, "2", "b");
//   }
//
//   let sv_1 = collab_1.transact().state_vector();
//   let sv_update = collab_2.transact().encode_state_as_update_v1(&sv_1);
//   {
//     let mut txn = collab_1.transact_mut();
//     let update = Update::decode_v1(&sv_update).unwrap();
//     txn.apply_update(update);
//   }
//   let a = {
//     let txn = collab_1.transact();
//     map_1.to_json(&txn)
//   };
//
//   let b = {
//     let txn = collab_1.transact();
//     collab_1.to_json(&txn)
//   };
//   let c = {
//     let txn = collab_2.transact();
//     map_2.to_json(&txn)
//   };
//
//   println!("a: {}", a);
//   println!("b: {}", b);
//   println!("c: {}", c);
// }
