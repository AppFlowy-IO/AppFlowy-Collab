#![allow(clippy::all)]

use assert_json_diff::assert_json_eq;
use collab::core::collab::CollabBuilder;
use collab::core::origin::CollabOrigin;

use collab::preclude::{Collab, CollabPlugin};
use serde_json::json;

use std::sync::{Arc, Mutex};
use yrs::updates::decoder::Decode;

use yrs::ArrayPrelim;
use yrs::Map;

use yrs::types::ToJson;
use yrs::MapRef;
use yrs::ReadTxn;
use yrs::TransactionMut;
use yrs::Update;

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

  collab_1.insert("1", "a");
  collab_1.insert("2", "b");
  collab_1.insert("3", "c");

  let updates = plugin.take_updates();
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

  let mut updates = plugin.take_updates();
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
  assert!(collab_2.transact().store().pending_update().is_none());

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
  init_sync(&mut client_1, &server);
  init_sync(&mut client_2, &server);

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
  assert_eq!(
    client_1.to_json_value(),
    json!({"1": "a", "2": "b", "3": "c", "4": "d", "5": "e"}),
    "client 1 should insert 5 entries"
  );

  // Verify that client_1 has generated five updates.
  let client_1_updates = client_1_plugin.take_updates();
  assert_eq!(client_1_updates.len(), 5);

  // Split the updates into two parts and simulate partial reception by the server.
  let (first, second) = client_1_updates.split_at(3);
  server.with_origin_transact_mut(|txn| {
    for update in first {
      let update = Update::decode_v1(&update).unwrap();
      txn.apply_update(update);
    }
  });
  assert_eq!(
    server.to_json_value(),
    json!({"1": "a", "2": "b", "3": "c"}),
    "server applied first 3 updates"
  );

  // Simulate that the first server update is not applied, to mimic a missed broadcast.
  let first_server_updates = server_plugin.take_updates();
  assert_eq!(first_server_updates.len(), 1);

  // Server applies the second part of updates.
  server.with_origin_transact_mut(|txn| {
    for update in second {
      let update = Update::decode_v1(&update).unwrap();
      txn.apply_update(update);
    }
  });
  assert_eq!(
    server.to_json_value(),
    json!( {"1": "a", "2": "b", "3": "c", "4": "d", "5": "e"}),
    "server applied remaining 2 updates, having a complete state now"
  );

  let second_server_updates = server_plugin.take_updates();
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
  assert_eq!(
    client_2.to_json_value(),
    json!({}),
    "client 2 has missing updates"
  );

  // Encode the missing state as an update and apply it to client 2 to resolve the missing updates.
  let missing_update = Update::decode_v1(
    &server
      .transact()
      .encode_state_as_update_v1(&client_2.transact().state_vector()),
  )
  .unwrap();

  client_2.with_origin_transact_mut(|txn| txn.apply_update(missing_update));

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
async fn simulate_client_missing_server_broadcast_data_test2() {
  // Initialize clients and server with the same origin and test conditions.
  let mut client_1 = Collab::new_with_origin(CollabOrigin::Empty, "test".to_string(), vec![], true);
  client_1.initialize();
  let plugin_1 = ReceiveUpdatesPlugin::default();
  client_1.add_plugin(Box::new(plugin_1.clone()));
  client_1.insert("1", "a".to_string());
  client_1.insert("2", "b".to_string());
  client_1.insert("3", "c".to_string());

  let mut client_2 = Collab::new_with_origin(CollabOrigin::Empty, "test".to_string(), vec![], true);
  client_2.initialize();
  let plugin_2 = ReceiveUpdatesPlugin::default();
  client_2.add_plugin(Box::new(plugin_2.clone()));
  client_2.insert("4", "d".to_string());
  client_2.insert("5", "e".to_string());
  client_2.insert("6", "f".to_string());

  let update_1 = plugin_1.take_updates();
  let update_2 = plugin_2.take_updates();

  let mut server = Collab::new_with_origin(CollabOrigin::Empty, "test".to_string(), vec![], false);
  server.initialize();

  // Split the updates into two parts and simulate partial reception by the server.
  let (first_1, second_1) = update_1.split_at(2);
  let (first_2, _second_2) = update_2.split_at(2);

  // the second_1 updates will be deprecated when applying other client's update
  server.with_origin_transact_mut(|txn| {
    for update in second_1 {
      let update = Update::decode_v1(&update).unwrap();
      txn.apply_update(update);
    }
  });
  // applied: {3:c} (pending), missing: {1:a, 2:b}

  // before the first_1 is not applied, so there is a pending update
  assert!(server.transact().store().pending_update().is_some());

  // apply the first_1 updates. after applying the first_1 updates, the pending update is none
  server.with_origin_transact_mut(|txn| {
    for update in first_2 {
      let update = Update::decode_v1(&update).unwrap();
      txn.apply_update(update);
    }
  });
  // applied: {4:d,5:e}, pending: {3:c}, missing: {1:a, 2:b}
  assert_json_eq!(
    server.to_json_value(),
    json!({
      "4": "d",
      "5": "e"
    })
  );
  assert!(server.transact().store().pending_update().is_some());

  // the second_2 updates was deprecated
  server.with_origin_transact_mut(|txn| {
    for update in first_1 {
      let update = Update::decode_v1(&update).unwrap();
      txn.apply_update(update);
    }
  });
  // applied: {1:a,2:b,4:d,5:e}, re-applied: {3:c}
  assert_json_eq!(
    server.to_json_value(),
    json!({
      "1": "a",
      "2": "b",
      "3": "c",
      "4": "d",
      "5": "e"
    })
  );
  server.with_origin_transact_mut(|txn| {
    for update in second_1 {
      let update = Update::decode_v1(&update).unwrap();
      txn.apply_update(update);
    }
  });
  // update {6:f} was never applied
  assert_json_eq!(
    server.to_json_value(),
    json!( {
      "1": "a",
      "2": "b",
      "3": "c",
      "4": "d",
      "5": "e"
    }),
  );
}

#[tokio::test]
async fn init_sync_test() {
  let mut client_1 =
    Collab::new_with_origin(CollabOrigin::Empty, "test".to_string(), vec![], false);
  client_1.initialize();

  // client 1 edit
  {
    let mut txn = client_1.transact_mut();
    client_1
      .insert_json_with_path(&mut txn, ["map"], json!({}))
      .unwrap();
    let outer_map: MapRef = client_1.get_with_path(&txn, ["map"]).unwrap();
    outer_map.insert(&mut txn, "1", "a");
    outer_map.insert(&mut txn, "array", ArrayPrelim::default());
  }

  let mut client_2 =
    Collab::new_with_origin(CollabOrigin::Empty, "test".to_string(), vec![], false);
  client_2.initialize();

  let mut server_collab =
    Collab::new_with_origin(CollabOrigin::Empty, "test".to_string(), vec![], false);
  server_collab.initialize();

  init_sync(&mut server_collab, &client_1);
  init_sync(&mut client_2, &server_collab);

  assert_eq!(client_1.to_json(), server_collab.to_json());
  assert_eq!(client_2.to_json(), server_collab.to_json());
}

fn init_sync(destination: &mut Collab, source: &Collab) {
  let source_tx = source.transact();
  let mut dest_tx = destination.transact_mut();

  let timestamp = dest_tx.state_vector();
  let update = source_tx.encode_state_as_update_v1(&timestamp);
  let update = Update::decode_v1(&update).unwrap();
  dest_tx.apply_update(update);
}

#[tokio::test]
async fn restore_from_multiple_update() {
  let update_cache = CollabStateCachePlugin::new();
  let mut collab = CollabBuilder::new(1, "1")
    .with_device_id("1")
    .with_plugin(update_cache.clone())
    .build()
    .unwrap();
  collab.initialize();

  // Insert map
  {
    let mut tx = collab.transact_mut();
    collab
      .insert_json_with_path(
        &mut tx,
        ["bullet"],
        json!({
          "1": "task 1",
          "2": "task 2"
        }),
      )
      .unwrap();
  }

  let updates = update_cache.get_doc_state().unwrap();
  let restored_collab = CollabBuilder::new(1, "1")
    .with_device_id("1")
    .with_doc_state(updates)
    .build()
    .unwrap();
  assert_eq!(collab.to_json(), restored_collab.to_json());
}

#[tokio::test]
async fn apply_same_update_multiple_time() {
  let update_cache = CollabStateCachePlugin::new();
  let mut collab = CollabBuilder::new(1, "1")
    .with_device_id("1")
    .with_plugin(update_cache.clone())
    .build()
    .unwrap();
  collab.initialize();
  collab.insert("text", "hello world");

  let updates = update_cache.get_doc_state().unwrap();
  let mut restored_collab = CollabBuilder::new(1, "1")
    .with_device_id("1")
    .with_doc_state(updates)
    .build()
    .unwrap();

  // It's ok to apply the updates that were already applied
  let doc_state = update_cache.get_doc_state().unwrap();
  restored_collab.with_origin_transact_mut(|txn| {
    let update = doc_state.as_update().unwrap().unwrap();
    txn.apply_update(update);
  });

  assert_json_eq!(collab.to_json(), restored_collab.to_json());
}

#[tokio::test]
async fn root_change_test() {
  setup_log();
  let mut collab_1 = CollabBuilder::new(1, "1")
    .with_device_id("1")
    .build()
    .unwrap();
  collab_1.initialize();
  let mut collab_2 = CollabBuilder::new(1, "1")
    .with_device_id("1")
    .build()
    .unwrap();
  collab_2.initialize();

  {
    collab_1
      .insert_json_with_path(&mut collab_1.transact_mut(), ["map"], json!({}))
      .unwrap();
  }
  {
    collab_2
      .insert_json_with_path(&mut collab_2.transact_mut(), ["map"], json!({}))
      .unwrap();
  }

  let map_2 = {
    let mut txn = collab_2.transact_mut();
    let map_2: MapRef = collab_2.get_with_path(&txn, ["map"]).unwrap();
    map_2.insert(&mut txn, "1", "a");
    map_2.insert(&mut txn, "2", "b");
    map_2
  };

  let sv_1 = collab_1.transact().state_vector();
  let sv_1_update = collab_2.transact().encode_state_as_update_v1(&sv_1);
  let sv_1_update = Update::decode_v1(&sv_1_update).unwrap();

  let map_1: MapRef = {
    collab_1.with_origin_transact_mut(|txn| {
      txn.apply_update(sv_1_update);
    });

    collab_1
      .get_with_path(&collab_1.transact(), ["map"])
      .unwrap()
  };

  let a = map_1.to_json(&collab_1.transact());
  let b = map_2.to_json(&collab_2.transact());

  assert_eq!(a, b);
}

#[derive(Clone, Default)]
struct ReceiveUpdatesPlugin {
  updates: Arc<Mutex<Vec<Vec<u8>>>>,
}

impl ReceiveUpdatesPlugin {
  fn take_updates(&self) -> Vec<Vec<u8>> {
    std::mem::take(&mut *self.updates.lock().unwrap())
  }
}

impl CollabPlugin for ReceiveUpdatesPlugin {
  fn receive_update(&self, _object_id: &str, _txn: &TransactionMut, update: &[u8]) {
    self.updates.lock().unwrap().push(update.to_vec());
  }
}
