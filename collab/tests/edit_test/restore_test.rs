#![allow(clippy::all)]

use assert_json_diff::assert_json_eq;
use collab::core::collab::{CollabOptions, DataSource, default_client_id};
use collab::core::origin::CollabOrigin;

use collab::preclude::{Collab, CollabPlugin, MapExt};
use serde_json::json;

use std::sync::{Arc, Mutex};
use yrs::updates::decoder::Decode;

use yrs::Map;
use yrs::{ArrayPrelim, ReadTxn};

use crate::util::{CollabStateCachePlugin, setup_log};
use collab::core::collab_plugin::CollabPluginType;
use yrs::MapRef;
use yrs::TransactionMut;
use yrs::Update;
use yrs::types::ToJson;

#[tokio::test]
async fn restore_from_update() {
  let options = CollabOptions::new("test".to_string(), default_client_id());
  let mut c1 = Collab::new_with_options(CollabOrigin::Empty, options).unwrap();
  let plugin = ReceiveUpdatesPlugin::default();
  c1.add_plugin(Box::new(plugin.clone()));
  c1.initialize();

  let options = CollabOptions::new("test".to_string(), default_client_id());
  let mut c2 = Collab::new_with_options(CollabOrigin::Empty, options).unwrap();
  c2.initialize();

  c1.insert("1", "a");
  c1.insert("2", "b");
  c1.insert("3", "c");

  let updates = plugin.take_updates();
  {
    let mut txn = c2.transact_mut();
    for update in updates {
      let update = Update::decode_v1(&update).unwrap();
      txn.apply_update(update).unwrap();
    }
  }

  assert_eq!(c1.to_json(), c2.to_json());
}

#[tokio::test]
async fn missing_update_test() {
  let options = CollabOptions::new("test".to_string(), default_client_id());
  let mut c1 = Collab::new_with_options(CollabOrigin::Empty, options).unwrap();
  let plugin = ReceiveUpdatesPlugin::default();
  c1.add_plugin(Box::new(plugin.clone()));
  c1.initialize();

  let options = CollabOptions::new("test".to_string(), default_client_id());
  let mut c2 = Collab::new_with_options(CollabOrigin::Empty, options).unwrap();
  c2.initialize();

  c1.insert("1", "a".to_string());
  c1.insert("2", "b".to_string());
  c1.insert("3", "c".to_string());
  c1.insert("4", "d".to_string());
  c1.insert("5", "e".to_string());

  let mut updates = plugin.take_updates();
  assert_eq!(updates.len(), 5);
  // simulate lost one update
  updates.remove(1);
  updates.remove(2);

  {
    let mut txn = c2.transact_mut();
    for update in updates {
      let update = Update::decode_v1(&update).unwrap();
      txn.apply_update(update).unwrap();
    }
    assert!(txn.store().pending_update().is_some())
  }

  // query the store state, testing if there are some pending updates
  assert!(c2.transact().store().pending_update().is_some());
  let missing_update = {
    c1.transact()
      .encode_state_as_update_v1(&c2.transact().store().pending_update().unwrap().missing)
  };

  let update = Update::decode_v1(&missing_update).unwrap();
  c2.apply_update(update).unwrap();

  assert!(c2.transact().store().pending_update().is_none());

  assert_eq!(c1.to_json_value(), c2.to_json_value());
}

/// Test to ensure that missing updates are correctly handled in a collaborative environment.
///
/// This tests simulates a scenario with two clients (`client_1` and `client_2`) and a server (`server`).
/// `client_1` sends updates to the server which are partially received by `client_2`.
/// The goal is to test the synchronization logic when `client_2` misses some updates initially received by the server.
#[tokio::test]
async fn simulate_client_missing_server_broadcast_data_test() {
  // Initialize clients and server with the same origin and test conditions.
  let options = CollabOptions::new("test".to_string(), default_client_id());
  let mut c1 = Collab::new_with_options(CollabOrigin::Empty, options).unwrap();
  c1.initialize();

  let options = CollabOptions::new("test".to_string(), default_client_id());
  let mut c2 = Collab::new_with_options(CollabOrigin::Empty, options).unwrap();
  c2.initialize();

  let options = CollabOptions::new("test".to_string(), default_client_id());
  let mut server = Collab::new_with_options(CollabOrigin::Empty, options).unwrap();
  server.initialize();

  // Perform initial synchronization to simulate starting conditions.
  init_sync(&mut c1, &server);
  init_sync(&mut c2, &server);

  // Plugins to capture updates for testing.
  let client_1_plugin = ReceiveUpdatesPlugin::default();
  c1.add_plugin(Box::new(client_1_plugin.clone()));

  let server_plugin = ReceiveUpdatesPlugin::default();
  server.add_plugin(Box::new(server_plugin.clone()));

  // Simulate client_1 sending multiple updates to the server.
  c1.insert("1", "a".to_string());
  c1.insert("2", "b".to_string());
  c1.insert("3", "c".to_string());
  c1.insert("4", "d".to_string());
  c1.insert("5", "e".to_string());
  assert_eq!(
    c1.to_json_value(),
    json!({"1": "a", "2": "b", "3": "c", "4": "d", "5": "e"}),
    "client 1 should insert 5 entries"
  );

  // Verify that client_1 has generated five updates.
  let client_1_updates = client_1_plugin.take_updates();
  assert_eq!(client_1_updates.len(), 5);

  // Split the updates into two parts and simulate partial reception by the server.
  let (first, second) = client_1_updates.split_at(3);
  {
    let mut txn = server.transact_mut();
    for update in first {
      let update = Update::decode_v1(&update).unwrap();
      txn.apply_update(update).unwrap();
    }
  }
  assert_eq!(
    server.to_json_value(),
    json!({"1": "a", "2": "b", "3": "c"}),
    "server applied first 3 updates"
  );

  // Simulate that the first server update is not applied, to mimic a missed broadcast.
  let first_server_updates = server_plugin.take_updates();
  assert_eq!(first_server_updates.len(), 1);

  // Server applies the second part of updates.
  {
    let mut txn = server.transact_mut();
    for update in second {
      let update = Update::decode_v1(&update).unwrap();
      txn.apply_update(update).unwrap();
    }
  }
  assert_eq!(
    server.to_json_value(),
    json!( {"1": "a", "2": "b", "3": "c", "4": "d", "5": "e"}),
    "server applied remaining 2 updates, having a complete state now"
  );

  let second_server_updates = server_plugin.take_updates();
  assert_eq!(second_server_updates.len(), 1);

  // Simulate client 2 receiving the latter updates and missing the first one.
  {
    let mut txn = c2.transact_mut();
    for update in second_server_updates {
      let update = Update::decode_v1(&update).unwrap();
      txn.apply_update(update).unwrap();
    }
    // Verify that client 2 is now out of sync due to missing updates.
    assert!(txn.store().pending_update().is_some());
  }
  assert_eq!(
    c2.to_json_value(),
    json!({}),
    "client 2 has missing updates"
  );

  // Encode the missing state as an update and apply it to client 2 to resolve the missing updates.
  let missing_update = Update::decode_v1(
    &server
      .transact()
      .encode_state_as_update_v1(&c2.transact().state_vector()),
  )
  .unwrap();

  c2.apply_update(missing_update).unwrap();

  // Ensure all clients and the server have synchronized states.
  assert_eq!(c1.to_json_value(), c2.to_json_value());
  assert_eq!(c1.to_json_value(), server.to_json_value());

  // Final verification against a static expected JSON value.
  assert_json_eq!(
    c1.to_json_value(),
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
  let options = CollabOptions::new("test".to_string(), default_client_id());
  let mut client_1 = Collab::new_with_options(CollabOrigin::Empty, options).unwrap();
  client_1.initialize();
  let plugin_1 = ReceiveUpdatesPlugin::default();
  client_1.add_plugin(Box::new(plugin_1.clone()));
  client_1.insert("1", "a".to_string());
  client_1.insert("2", "b".to_string());
  client_1.insert("3", "c".to_string());

  let options = CollabOptions::new("test".to_string(), default_client_id());
  let mut client_2 = Collab::new_with_options(CollabOrigin::Empty, options).unwrap();
  client_2.initialize();
  let plugin_2 = ReceiveUpdatesPlugin::default();
  client_2.add_plugin(Box::new(plugin_2.clone()));
  client_2.insert("4", "d".to_string());
  client_2.insert("5", "e".to_string());
  client_2.insert("6", "f".to_string());

  let update_1 = plugin_1.take_updates();
  let update_2 = plugin_2.take_updates();

  let options = CollabOptions::new("test".to_string(), default_client_id());
  let mut server = Collab::new_with_options(CollabOrigin::Empty, options).unwrap();
  server.initialize();

  // Split the updates into two parts and simulate partial reception by the server.
  let (first_1, second_1) = update_1.split_at(2);
  let (first_2, _second_2) = update_2.split_at(2);

  // the second_1 updates will be deprecated when applying other client's update
  {
    let mut txn = server.transact_mut();
    for update in second_1 {
      let update = Update::decode_v1(&update).unwrap();
      txn.apply_update(update).unwrap();
    }
  }
  // applied: {3:c} (pending), missing: {1:a, 2:b}

  // before the first_1 is not applied, so there is a pending update
  assert!(server.transact().store().pending_update().is_some());

  // apply the first_1 updates. after applying the first_1 updates, the pending update is none
  {
    let mut txn = server.transact_mut();
    for update in first_2 {
      let update = Update::decode_v1(&update).unwrap();
      txn.apply_update(update).unwrap();
    }
  }
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
  {
    let mut txn = server.transact_mut();
    for update in first_1 {
      let update = Update::decode_v1(&update).unwrap();
      txn.apply_update(update).unwrap();
    }
  }
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
  {
    let mut txn = server.transact_mut();
    for update in second_1 {
      let update = Update::decode_v1(&update).unwrap();
      txn.apply_update(update).unwrap();
    }
  }
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
  let options = CollabOptions::new("test".to_string(), default_client_id());
  let mut client_1 = Collab::new_with_options(CollabOrigin::Empty, options).unwrap();
  client_1.initialize();

  // client 1 edit
  {
    let mut txn = client_1.context.transact_mut();
    client_1
      .data
      .insert_json_with_path(&mut txn, ["map"], json!({}))
      .unwrap();
    let outer_map: MapRef = client_1.data.get_with_path(&txn, ["map"]).unwrap();
    outer_map.insert(&mut txn, "1", "a");
    outer_map.insert(&mut txn, "array", ArrayPrelim::default());
  }

  let options = CollabOptions::new("test".to_string(), default_client_id());
  let mut client_2 = Collab::new_with_options(CollabOrigin::Empty, options).unwrap();
  client_2.initialize();

  let options = CollabOptions::new("test".to_string(), default_client_id());
  let mut server_collab = Collab::new_with_options(CollabOrigin::Empty, options).unwrap();
  server_collab.initialize();

  init_sync(&mut server_collab, &client_1);
  init_sync(&mut client_2, &server_collab);

  assert_eq!(client_1.to_json(), server_collab.to_json());
  assert_eq!(client_2.to_json(), server_collab.to_json());
}

fn init_sync(destination: &mut Collab, source: &Collab) {
  let source_tx = source.transact();
  let mut dest_tx = destination.context.transact_mut();

  let timestamp = dest_tx.state_vector();
  let update = source_tx.encode_state_as_update_v1(&timestamp);
  let update = Update::decode_v1(&update).unwrap();
  dest_tx.apply_update(update).unwrap();
}

#[tokio::test]
async fn restore_from_multiple_update() {
  let update_cache = CollabStateCachePlugin::new();
  let options = CollabOptions::new("1".to_string(), default_client_id())
    .with_data_source(DataSource::Disk(None));
  let mut collab = Collab::new_with_options(CollabOrigin::Empty, options).unwrap();
  collab.add_plugin(Box::new(update_cache.clone()));
  collab.initialize();

  // Insert map
  {
    let mut tx = collab.context.transact_mut();
    collab
      .data
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
  let options = CollabOptions::new("1".to_string(), default_client_id()).with_data_source(updates);
  let restored_collab = Collab::new_with_options(CollabOrigin::Empty, options).unwrap();
  assert_eq!(collab.to_json(), restored_collab.to_json());
}

#[tokio::test]
async fn apply_same_update_multiple_time() {
  let update_cache = CollabStateCachePlugin::new();
  let options = CollabOptions::new("1".to_string(), default_client_id())
    .with_data_source(DataSource::Disk(None));
  let mut collab = Collab::new_with_options(CollabOrigin::Empty, options).unwrap();
  collab.add_plugin(Box::new(update_cache.clone()));
  collab.initialize();
  collab.insert("text", "hello world");

  let updates = update_cache.get_doc_state().unwrap();
  let options = CollabOptions::new("1".to_string(), default_client_id()).with_data_source(updates);
  let mut restored_collab = Collab::new_with_options(CollabOrigin::Empty, options).unwrap();

  // It's ok to apply the updates that were already applied
  let doc_state = update_cache.get_doc_state().unwrap();
  let update = doc_state.as_update().unwrap().unwrap();
  restored_collab.apply_update(update).unwrap();

  assert_json_eq!(collab.to_json(), restored_collab.to_json());
}

#[ignore = "fixme: flaky test"]
#[tokio::test]
async fn root_change_test() {
  setup_log();
  let options = CollabOptions::new("1".to_string(), default_client_id())
    .with_data_source(DataSource::Disk(None));
  let mut collab_1 = Collab::new_with_options(CollabOrigin::Empty, options).unwrap();
  collab_1.initialize();

  let options = CollabOptions::new("1".to_string(), default_client_id())
    .with_data_source(DataSource::Disk(None));
  let mut collab_2 = Collab::new_with_options(CollabOrigin::Empty, options).unwrap();
  collab_2.initialize();

  {
    collab_1
      .data
      .insert_json_with_path(&mut collab_1.context.transact_mut(), ["map"], json!({}))
      .unwrap();
  }
  {
    collab_2
      .data
      .insert_json_with_path(&mut collab_2.context.transact_mut(), ["map"], json!({}))
      .unwrap();
  }

  let map_2 = {
    let mut txn = collab_2.context.transact_mut();
    let map_2: MapRef = collab_2.data.get_with_path(&txn, ["map"]).unwrap();
    map_2.insert(&mut txn, "1", "a");
    map_2.insert(&mut txn, "2", "b");
    map_2
  };

  let sv_1 = collab_1.transact().state_vector();
  let sv_1_update = collab_2.transact().encode_state_as_update_v1(&sv_1);
  let sv_1_update = Update::decode_v1(&sv_1_update).unwrap();

  let map_1: MapRef = {
    collab_1.apply_update(sv_1_update).unwrap();

    collab_1
      .data
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

  fn plugin_type(&self) -> CollabPluginType {
    CollabPluginType::Other("ReceiveUpdatesPlugin".to_string())
  }
}
