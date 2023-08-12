#![allow(clippy::all)]

use std::collections::HashMap;

use collab::core::collab::CollabBuilder;
use collab::preclude::MapRefExtension;
use yrs::types::ToJson;
use yrs::updates::decoder::Decode;
use yrs::{Doc, Map, MapPrelim, ReadTxn, Transact, Update};

use crate::helper::{setup_log, CollabStateCachePlugin};

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

  let updates = update_cache.get_updates().unwrap();
  let restored_collab = CollabBuilder::new(1, "1")
    .with_device_id("1")
    .with_raw_data(updates)
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

  let updates = update_cache.get_updates().unwrap();
  let restored_collab = CollabBuilder::new(1, "1")
    .with_device_id("1")
    .with_raw_data(updates)
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

  let updates = update_cache.get_updates().unwrap();
  let restored_collab = CollabBuilder::new(1, "1")
    .with_device_id("1")
    .with_raw_data(updates)
    .build()
    .unwrap();

  // It's ok to apply the updates that were already applied
  let updates = update_cache.get_updates().unwrap();
  restored_collab.lock().with_origin_transact_mut(|txn| {
    for update in updates {
      txn.apply_update(Update::decode_v1(&update).unwrap());
    }
  });

  assert_json_diff::assert_json_eq!(collab.lock().to_json(), restored_collab.lock().to_json(),);
}

#[tokio::test]
async fn apply_unordered_updates() {
  let update_cache = CollabStateCachePlugin::new();
  let collab = CollabBuilder::new(1, "1")
    .with_device_id("1")
    .with_plugin(update_cache.clone())
    .build()
    .unwrap();
  collab.lock().initialize();
  collab.lock().insert("text", "hello world");

  // Insert map
  let mut map = HashMap::new();
  map.insert("1".to_string(), "task 1".to_string());
  map.insert("2".to_string(), "task 2".to_string());
  collab.lock().insert("bullet", map);

  let mut updates = update_cache.get_updates().unwrap();
  updates.reverse();

  let restored_collab = CollabBuilder::new(1, "1")
    .with_device_id("1")
    .build()
    .unwrap();
  restored_collab.lock().initialize();
  restored_collab.lock().with_origin_transact_mut(|txn| {
    //Out of order updates from the same peer will be stashed internally and their
    // integration will be postponed until missing blocks arrive first.
    for update in updates {
      txn.apply_update(Update::decode_v1(&update).unwrap());
    }
  });

  assert_json_diff::assert_json_eq!(
    serde_json::json!( {
      "text": "hello world"
    }),
    restored_collab.to_json_value()
  );
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

  let a = map_1.to_json_value();
  let b = map_2.to_json_value();

  println!("a: {}", a);
  println!("b: {}", b);
  // assert_eq!(a, b);
}

// The result is undetermined because the two peers are in a different state. Check out the
// two_way_sync_test for a more detailed explanation.
#[tokio::test]
async fn two_way_sync_result_undetermined() {
  let doc_1 = Doc::new();
  let doc_2 = Doc::new();
  let root_map_1 = doc_1.get_or_insert_map("root");
  let root_map_2 = doc_2.get_or_insert_map("root");

  // root: { map:{ } }
  let _map_1 = {
    let mut txn = doc_1.transact_mut();
    root_map_1.insert(&mut txn, "map", MapPrelim::<lib0::any::Any>::new())
  };

  // root: { map:{ } }
  let map_2 = {
    let mut txn = doc_2.transact_mut();
    root_map_2.insert(&mut txn, "map", MapPrelim::<lib0::any::Any>::new())
  };

  {
    let mut txn = doc_2.transact_mut();
    map_2.insert(&mut txn, "key_1", "a");
    map_2.insert(&mut txn, "key_2", "b");
  }

  {
    let sv_1 = doc_1.transact().state_vector();
    let sv_update = doc_2.transact().encode_state_as_update_v1(&sv_1);
    let mut txn = doc_1.transact_mut();
    let update = Update::decode_v1(&sv_update).unwrap();
    txn.apply_update(update);
  }

  // When synchronizing updates, what happens is that a conflict has occurred - under the same key
  // "map" two different maps where inserted - map_1 and map_2 are logically different entities (in
  // Yjs/Yrs only root types are logically equivalent by their name). In order to resolve this conflict,
  // an update that created a nested map from the client with higher ID will override the one that came
  // from client with lower ID. If that happens, the overridden map will be tombstoned together with
  // all its elements.
  //
  // That Doc::new() generates random client ID for the document. So the two way sync is reuqired
  {
    let sv_2 = doc_2.transact().state_vector();
    let sv_update = doc_1.transact().encode_state_as_update_v1(&sv_2);
    let mut txn = doc_2.transact_mut();
    let update = Update::decode_v1(&sv_update).unwrap();
    txn.apply_update(update);
  }

  // The a and b must be the same and might be empty. This is the result of the two way sync.
  let a = {
    let txn = doc_1.transact();
    root_map_1.to_json(&txn)
  };

  let b = {
    let txn = doc_2.transact();
    root_map_2.to_json(&txn)
  };

  println!("a: {}", a);
  println!("b: {}", b);
  assert_eq!(a, b);
}

#[tokio::test]
async fn two_way_sync_test() {
  let doc_1 = Doc::new();
  let doc_2 = Doc::new();
  let root_map_1 = doc_1.get_or_insert_map("root");
  let root_map_2 = doc_2.get_or_insert_map("root");

  // root: { map:{ } }
  let _map_1 = {
    let mut txn = doc_1.transact_mut();
    root_map_1.insert(&mut txn, "map", MapPrelim::<lib0::any::Any>::new())
  };

  // sync the doc_1 local state to doc_2. Then the "map" will be treated as the same object.
  {
    let sv_1 = doc_1.transact().state_vector();
    let sv_update = doc_2.transact().encode_state_as_update_v1(&sv_1);
    let mut txn = doc_1.transact_mut();
    let update = Update::decode_v1(&sv_update).unwrap();
    txn.apply_update(update);
  }
  {
    let sv_2 = doc_2.transact().state_vector();
    let sv_update = doc_1.transact().encode_state_as_update_v1(&sv_2);
    let mut txn = doc_2.transact_mut();
    let update = Update::decode_v1(&sv_update).unwrap();
    txn.apply_update(update);
  }

  // Update the "map" in doc_2 and then sync to doc_1
  let map_2 = {
    let txn = doc_2.transact();
    root_map_2.get_map_with_txn(&txn, "map").unwrap()
  };
  {
    let mut txn = doc_2.transact_mut();
    map_2.insert(&mut txn, "key_1", "a");
    map_2.insert(&mut txn, "key_2", "b");
  }
  {
    let sv_1 = doc_1.transact().state_vector();
    let sv_update = doc_2.transact().encode_state_as_update_v1(&sv_1);
    let mut txn = doc_1.transact_mut();
    let update = Update::decode_v1(&sv_update).unwrap();
    txn.apply_update(update);
  }
  {
    let sv_2 = doc_2.transact().state_vector();
    let sv_update = doc_1.transact().encode_state_as_update_v1(&sv_2);
    let mut txn = doc_2.transact_mut();
    let update = Update::decode_v1(&sv_update).unwrap();
    txn.apply_update(update);
  }

  // The a and b must be the same and not empty
  let a = {
    let txn = doc_1.transact();
    root_map_1.to_json(&txn)
  };

  let b = {
    let txn = doc_2.transact();
    root_map_2.to_json(&txn)
  };

  println!("a: {}", a);
  println!("b: {}", b);
  assert_eq!(a, b);
}
