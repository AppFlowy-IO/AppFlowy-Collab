use std::collections::HashMap;

use collab::core::collab::CollabBuilder;
use serde_json::json;

use crate::helper::CollabStateCachePlugin;

#[test]
fn restore_from_update() {
  let update_cache = CollabStateCachePlugin::new();
  let collab = CollabBuilder::new(1, "1")
    .with_plugin(update_cache.clone())
    .build();
  collab.lock().initial();
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
  collab.lock().initial();
  collab.lock().insert("text", "hello world");

  // Insert map
  let mut map = HashMap::new();
  map.insert("1".to_string(), "task 1".to_string());
  map.insert("2".to_string(), "task 2".to_string());
  collab.lock().insert("bullet", map);

  let mut updates = update_cache.get_updates().unwrap();
  updates.reverse();

  let restored_collab = CollabBuilder::new(1, "1").build();
  restored_collab.lock().initial();
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
