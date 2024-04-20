#![allow(clippy::all)]

use assert_json_diff::assert_json_eq;
use collab::core::collab::{CollabBuilder, DataSource};
use collab::core::origin::CollabOrigin;
use collab::preclude::{Collab, CollabPlugin};
use parking_lot::RwLock;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use yrs::updates::decoder::Decode;
use yrs::{ReadTxn, Transact, TransactionMut, Update};

use crate::util::{setup_log, CollabStateCachePlugin};

#[tokio::test]
async fn restore_from_update() {
  let mut collab_1 =
    Collab::new_with_origin(CollabOrigin::Empty, "test".to_string(), vec![], false);
  let plugin = ReceiveUpdatesPlugin::default();
  collab_1.add_plugin(Box::new(plugin.clone()));
  collab_1.initialize();

  let mut collab_2 =
    Collab::new_with_origin(CollabOrigin::Empty, "test".to_string(), vec![], false);
  collab_2.initialize();

  collab_1.insert("1", "a".to_string());
  collab_1.insert("2", "b".to_string());
  collab_1.insert("3", "c".to_string());

  let updates = std::mem::take(&mut *plugin.updates.write());
  collab_2.with_origin_transact_mut(|txn| {
    for update in updates {
      let update = Update::decode_v1(&update).unwrap();
      txn.apply_update(update);
    }
  });

  assert_eq!(collab_1.to_json(), collab_2.to_json());
}

#[tokio::test]
async fn missing_update_test() {
  let mut collab_1 =
    Collab::new_with_origin(CollabOrigin::Empty, "test".to_string(), vec![], false);
  let plugin = ReceiveUpdatesPlugin::default();
  collab_1.add_plugin(Box::new(plugin.clone()));
  collab_1.initialize();

  let mut collab_2 =
    Collab::new_with_origin(CollabOrigin::Empty, "test".to_string(), vec![], false);
  collab_2.initialize();

  collab_1.insert("1", "a".to_string());
  collab_1.insert("2", "b".to_string());
  collab_1.insert("3", "c".to_string());
  collab_1.insert("4", "d".to_string());
  collab_1.insert("5", "e".to_string());

  let mut updates = std::mem::take(&mut *plugin.updates.write());
  assert_eq!(updates.len(), 5);
  // simulate lost one update
  updates.remove(1);
  updates.remove(2);

  collab_2.with_origin_transact_mut(|txn| {
    for update in updates {
      let update = Update::decode_v1(&update).unwrap();
      txn.apply_update(update);
    }
    assert!(txn.store().pending_update().is_some())
  });

  // query the store state, testing if there are some pending updates
  assert!(collab_2.transact().store().pending_update().is_some());
  let missing_update = {
    collab_1.transact().encode_state_as_update_v1(
      &collab_2
        .transact()
        .store()
        .pending_update()
        .unwrap()
        .missing,
    )
  };

  collab_2.with_origin_transact_mut(|txn| {
    let update = Update::decode_v1(&missing_update).unwrap();
    txn.apply_update(update);
  });

  assert_eq!(collab_1.to_json_value(), collab_2.to_json_value());
}

/// Test to ensure that missing updates are correctly handled in a collaborative environment.
///
/// This tests simulates a scenario with two clients (`client_1` and `client_2`) and a server (`server`).
/// `client_1` sends updates to the server which are partially received by `client_2`.
/// The goal is to test the synchronization logic when `client_2` misses some updates initially received by the server.
#[tokio::test]
async fn simulate_client_missing_server_broadcast_data_test() {
  // Initialize clients and server with the same origin and test conditions.
  let mut client_1 =
    Collab::new_with_origin(CollabOrigin::Empty, "test".to_string(), vec![], false);
  client_1.initialize();
  let mut client_2 =
    Collab::new_with_origin(CollabOrigin::Empty, "test".to_string(), vec![], false);
  client_2.initialize();
  let mut server = Collab::new_with_origin(CollabOrigin::Empty, "test".to_string(), vec![], false);
  server.initialize();

  // Perform initial synchronization to simulate starting conditions.
  init_sync(&client_1, &server);
  init_sync(&client_2, &server);

  // Plugins to capture updates for testing.
  let client_1_plugin = ReceiveUpdatesPlugin::default();
  client_1.add_plugin(Box::new(client_1_plugin.clone()));

  let server_plugin = ReceiveUpdatesPlugin::default();
  server.add_plugin(Box::new(server_plugin.clone()));

  // Simulate client_1 sending multiple updates to the server.
  client_1.insert("1", "a".to_string());
  client_1.insert("2", "b".to_string());
  client_1.insert("3", "c".to_string());
  client_1.insert("4", "d".to_string());
  client_1.insert("5", "e".to_string());

  // Verify that client_1 has generated five updates.
  let client_1_updates = std::mem::take(&mut *client_1_plugin.updates.write());
  assert_eq!(client_1_updates.len(), 5);

  // Split the updates into two parts and simulate partial reception by the server.
  let (first, second) = client_1_updates.split_at(3);
  server.with_origin_transact_mut(|txn| {
    for update in first {
      let update = Update::decode_v1(&update).unwrap();
      txn.apply_update(update);
    }
  });

  // Simulate that the first server update is not applied, to mimic a missed broadcast.
  let first_server_updates = std::mem::take(&mut *server_plugin.updates.write());
  assert_eq!(first_server_updates.len(), 1);

  // Server applies the second part of updates.
  server.with_origin_transact_mut(|txn| {
    for update in second {
      let update = Update::decode_v1(&update).unwrap();
      txn.apply_update(update);
    }
  });

  let second_server_updates = std::mem::take(&mut *server_plugin.updates.write());
  assert_eq!(second_server_updates.len(), 1);

  // Simulate client 2 receiving the latter updates and missing the first one.
  client_2.with_origin_transact_mut(|txn| {
    for update in second_server_updates {
      let update = Update::decode_v1(&update).unwrap();
      txn.apply_update(update);
    }

    // Verify that client 2 is now out of sync due to missing updates.
    assert!(txn.store().pending_update().is_some());
  });

  // Encode the missing state as an update and apply it to client 2 to resolve the missing updates.
  let missing_update = server.transact().encode_state_as_update_v1(
    &client_2
      .transact()
      .store()
      .pending_update()
      .unwrap()
      .missing,
  );

  client_2.with_origin_transact_mut(|txn| {
    let update = Update::decode_v1(&missing_update).unwrap();
    txn.apply_update(update);
  });

  // Ensure all clients and the server have synchronized states.
  assert_eq!(client_1.to_json_value(), client_2.to_json_value());
  assert_eq!(client_1.to_json_value(), server.to_json_value());

  // Final verification against a static expected JSON value.
  assert_json_eq!(
    client_1.to_json_value(),
    json!({
      "1": "a",
      "2": "b",
      "3": "c",
      "4": "d",
      "5": "e"
    })
  );
}

#[tokio::test]
async fn init_sync_test() {
  let mut client_1 =
    Collab::new_with_origin(CollabOrigin::Empty, "test".to_string(), vec![], false);
  client_1.initialize();

  // client 1 edit
  client_1.with_origin_transact_mut(|txn| {
    client_1.insert_map_with_txn(txn, "map");
  });
  client_1.with_origin_transact_mut(|txn| {
    let outer_map = client_1.get_map_with_txn(txn, vec!["map"]).unwrap();
    outer_map.insert_with_txn(txn, "1", "a");
    outer_map.insert_array_with_txn(txn, "array", Vec::<String>::new());
  });

  let mut client_2 =
    Collab::new_with_origin(CollabOrigin::Empty, "test".to_string(), vec![], false);
  client_2.initialize();

  let mut server_collab =
    Collab::new_with_origin(CollabOrigin::Empty, "test".to_string(), vec![], false);
  server_collab.initialize();

  init_sync(&server_collab, &client_1);
  init_sync(&client_2, &server_collab);

  assert_eq!(client_1.to_json(), server_collab.to_json());
  assert_eq!(client_2.to_json(), server_collab.to_json());
}

fn init_sync(old: &Collab, new: &Collab) {
  let sv = old.transact().state_vector();
  let update = new.transact().encode_state_as_update_v1(&sv);
  old.with_origin_transact_mut(|txn| {
    let update = Update::decode_v1(&update).unwrap();
    txn.apply_update(update);
  });
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
      DataSource::Disk => {
        panic!("doc state should not be empty")
      },
      DataSource::DocStateV1(doc_state) => {
        txn.apply_update(Update::decode_v1(&doc_state).unwrap());
      },
      DataSource::DocStateV2(doc_state) => {
        txn.apply_update(Update::decode_v2(&doc_state).unwrap());
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

#[derive(Clone, Default)]
struct ReceiveUpdatesPlugin {
  updates: Arc<RwLock<Vec<Vec<u8>>>>,
}

impl CollabPlugin for ReceiveUpdatesPlugin {
  fn receive_update(&self, _object_id: &str, _txn: &TransactionMut, update: &[u8]) {
    self.updates.write().push(update.to_vec());
  }
}
