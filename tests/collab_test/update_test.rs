use crate::helper::{Document, Owner};
use appflowy_collab::collab::{Collab, CollabBuilder};
use appflowy_collab::plugin::disk::CollabStateCachePlugin;
use indexmap::IndexMap;
use std::collections::HashMap;
use yrs::Doc;

#[test]
fn sync_document_edit() {
    let old_email = "nathan@appflowy.io";
    let new_email = "nathan@gmail.com";

    let (local, remote, update_cache) = make_pair();
    let path = vec!["document", "owner"];
    let mut map = local.get_map_with_path(path.clone()).unwrap();
    map.insert("email", new_email);

    let email = remote
        .get_map_with_path(path.clone())
        .unwrap()
        .get_str("email")
        .unwrap();
    assert_eq!(email, old_email);

    let update = update_cache.get_update().unwrap();
    remote.with_transact_mut(|txn| txn.apply_update(update));

    let email = remote
        .get_map_with_path(path.clone())
        .unwrap()
        .get_str("email")
        .unwrap();
    assert_eq!(email, new_email);
}

fn make_pair() -> (Collab, Collab, CollabStateCachePlugin) {
    let update_cache = CollabStateCachePlugin::new();
    let mut local_collab = CollabBuilder::new("1".to_string(), 1)
        .with_plugin(update_cache.clone())
        .build();
    // Insert document
    local_collab.insert_json_attr_with_path(vec![], "document", test_document());
    let remote_collab =
        CollabBuilder::from_updates("1".to_string(), 1, update_cache.get_updates().unwrap())
            .build();

    (local_collab, remote_collab, update_cache)
}

fn test_document() -> Document {
    let owner = Owner {
        name: "nathan".to_string(),
        email: "nathan@appflowy.io".to_string(),
    };

    let mut attributes = IndexMap::new();
    attributes.insert("1".to_string(), "task 1".to_string());
    attributes.insert("2".to_string(), "task 2".to_string());

    Document {
        name: "Hello world".to_string(),
        owner,
        created_at: 0,
        attributes,
    }
}

#[test]
fn restore_from_update() {
    let update_cache = CollabStateCachePlugin::new();
    let collab = CollabBuilder::new("1".to_string(), 1)
        .with_plugin(update_cache.clone())
        .build();

    let remote_collab = CollabBuilder::new("1".to_string(), 1).build();

    // Insert text
    collab.insert_attr("text", "hello world");

    // Insert map
    let mut map = HashMap::new();
    map.insert("1".to_string(), "task 1".to_string());
    map.insert("2".to_string(), "task 2".to_string());
    collab.insert_attr("bullet", map);

    let updates = update_cache.get_updates().unwrap();
    let restored_collab = CollabBuilder::from_updates("1".to_string(), 1, updates).build();
    let value = restored_collab.get_attr("text").unwrap();
    let s = value.to_string(&restored_collab.transact());
    assert_eq!(s, "hello world");
}
