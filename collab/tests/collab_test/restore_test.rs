use std::collections::HashMap;

use collab::core::collab::CollabBuilder;
use collab::preclude::MapRefExtension;
use serde_json::json;
use yrs::updates::decoder::Decode;
use yrs::{ReadTxn, Transact, Update};

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
    let collab_1_guard = collab_1.lock();
    collab_1_guard.with_transact_mut(|txn| {
      collab_1_guard.insert_map_with_txn(txn, "map");
    });
    drop(collab_1_guard);
  }
  {
    let collab_2_guard = collab_2.lock();
    collab_2_guard.with_transact_mut(|txn| {
      collab_2_guard.insert_map_with_txn(txn, "map");
    });
    drop(collab_2_guard);
  }

  let map_2 = {
    let collab_guard = collab_2.lock();
    let txn = collab_guard.transact();
    let map_2 = collab_guard.get_map_with_txn(&txn, vec!["map"]).unwrap();
    drop(txn);

    collab_guard.with_transact_mut(|txn| {
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
    collab_1_guard.with_transact_mut(|txn| {
      let update = Update::decode_v1(&sv_1_update).unwrap();
      txn.apply_update(update);
    });

    let txn = collab_1_guard.transact();
    collab_1_guard.get_map_with_txn(&txn, vec!["map"]).unwrap()
  };

  let a = map_1.to_json_value();
  let b = map_2.to_json_value();

  println!("a: {}", a);
  println!("b: {}", b);
  // assert_eq!(a, b);
}

// #[test]
// fn root_change_test2() {
//   let doc_1 = Doc::new();
//   let doc_2 = Doc::new();
//   let root_map_1 = doc_1.get_or_insert_map("root");
//   let root_map_2 = doc_2.get_or_insert_map("root");
//
//   // root: { map:{ } }
//   let mut map_1 = {
//     let mut txn = doc_1.transact_mut();
//     root_map_1.insert(&mut txn, "map", MapPrelim::<lib0::any::Any>::new())
//   };
//   // root: { map:{ } }
//   let mut map_2 = {
//     let mut txn = doc_2.transact_mut();
//     root_map_2.insert(&mut txn, "map", MapPrelim::<lib0::any::Any>::new())
//   };
//
//   // let cloned_map_1 = map_1.clone();
//   // let cloned_map_2 = map_2.clone();
//   // let map_1_sub = map_1.observe(move |txn, event| {
//   //   // Only set the root changed flag if the remote origin is different from the local origin.
//   //   println!(
//   //     "1 event target: {:?}, map: {:?}",
//   //     event.target(),
//   //     cloned_map_1
//   //   );
//   // });
//   // let map_2_sub = map_2.observe(move |txn, event| {
//   //   // Only set the root changed flag if the remote origin is different from the local origin.
//   //   println!(
//   //     "2 event target: {:?}, map: {:?}",
//   //     event.target(),
//   //     cloned_map_2
//   //   );
//   // });
//
//   {
//     let mut txn = doc_2.transact_mut();
//     map_2.insert(&mut txn, "key_1", "a");
//     map_2.insert(&mut txn, "key_2", "b");
//   }
//
//   let sv_1 = doc_1.transact().state_vector();
//   let sv_update = doc_2.transact().encode_state_as_update_v1(&sv_1);
//   {
//     let mut txn = doc_1.transact_mut();
//     let update = Update::decode_v1(&sv_update).unwrap();
//     txn.apply_update(update);
//   }
//
//   let a = {
//     let txn = doc_1.transact();
//     root_map_1.to_json(&txn)
//   };
//
//   let b = {
//     let txn = doc_2.transact();
//     root_map_2.to_json(&txn)
//   };
//
//   println!("a: {}", a);
//   println!("b: {}", b);
//   assert_eq!(a, b);
// }
