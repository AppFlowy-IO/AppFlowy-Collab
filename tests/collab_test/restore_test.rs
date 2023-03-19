use appflowy_collab::collab::CollabBuilder;
use appflowy_collab::plugin::persistence::CollabUpdateMemCache;
use std::collections::HashMap;

#[test]
fn restore_from_update() {
    let update_cache = CollabUpdateMemCache::new();
    let collab = CollabBuilder::new("1".to_string(), 1)
        .with_plugin(update_cache.clone())
        .build();
    collab.insert_attr("text", "hello world");

    let updates = update_cache.get_updates().unwrap();
    let restored_collab = CollabBuilder::from_updates("1".to_string(), 1, updates).build();
    let value = restored_collab.get_str("text").unwrap();
    assert_eq!(value, "hello world");
}

#[test]
fn restore_from_multiple_update() {
    let update_cache = CollabUpdateMemCache::new();
    let collab = CollabBuilder::new("1".to_string(), 1)
        .with_plugin(update_cache.clone())
        .build();

    // Insert text
    collab.insert_attr("text", "hello world");

    // Insert map
    let mut map = HashMap::new();
    map.insert("1".to_string(), "task 1".to_string());
    map.insert("2".to_string(), "task 2".to_string());
    collab.insert_attr("bullet", map);

    assert_eq!(
        collab.to_string(),
        r#"{"attributes":{"bullet":{"1":"task 1","2":"task 2"},"text":"hello world"}}"#
    );

    let updates = update_cache.get_updates().unwrap();
    let restored_collab = CollabBuilder::from_updates("1".to_string(), 1, updates).build();
    assert_eq!(
        restored_collab.to_string(),
        r#"{"attributes":{"bullet":{"1":"task 1","2":"task 2"},"text":"hello world"}}"#
    );
}

#[test]
fn apply_same_update_multiple_time() {
    let update_cache = CollabUpdateMemCache::new();
    let collab = CollabBuilder::new("1".to_string(), 1)
        .with_plugin(update_cache.clone())
        .build();
    collab.insert_attr("text", "hello world");

    let updates = update_cache.get_updates().unwrap();
    let restored_collab = CollabBuilder::from_updates("1".to_string(), 1, updates).build();

    // It's ok to apply the updates that were already applied
    let updates = update_cache.get_updates().unwrap();
    restored_collab.with_transact_mut(|txn| {
        for update in updates {
            txn.apply_update(update);
        }
    });

    assert_eq!(
        restored_collab.to_string(),
        r#"{"attributes":{"text":"hello world"}}"#
    );
}

#[test]
fn apply_unordered_updates() {
    let update_cache = CollabUpdateMemCache::new();
    let collab = CollabBuilder::new("1".to_string(), 1)
        .with_plugin(update_cache.clone())
        .build();
    collab.insert_attr("text", "hello world");

    // Insert map
    let mut map = HashMap::new();
    map.insert("1".to_string(), "task 1".to_string());
    map.insert("2".to_string(), "task 2".to_string());
    collab.insert_attr("bullet", map);

    let mut updates = update_cache.get_updates().unwrap();
    updates.reverse();

    let restored_collab = CollabBuilder::new("1".to_string(), 1).build();
    restored_collab.with_transact_mut(|txn| {
        for update in updates {
            txn.apply_update(update);
        }
    });

    assert_eq!(
        restored_collab.to_string(),
        r#"{"attributes":{"bullet":{"1":"task 1","2":"task 2"},"text":"hello world"}}"#
    );
}
