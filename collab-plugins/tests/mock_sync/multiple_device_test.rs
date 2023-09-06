use collab_plugins::sync_plugin::client::DEFAULT_SYNC_TIMEOUT;
use serde_json::json;
use yrs::Array;

use crate::util::ScriptTest;
use crate::util::TestScript::*;

#[tokio::test]
async fn three_clients_on_line_test() {
  let mut test = ScriptTest::new("1").await;
  test
    .run_scripts(vec![
      CreateClient {
        uid: 1,
        device_id: "1".to_string(),
      },
      CreateEmptyClient {
        uid: 1,
        device_id: "2".to_string(),
      },
      CreateEmptyClient {
        uid: 1,
        device_id: "3".to_string(),
      },
      ModifyLocalCollab {
        device_id: "1".to_string(),
        f: |collab| {
          collab.insert("1", "a");
        },
      },
      ModifyLocalCollab {
        device_id: "2".to_string(),
        f: |collab| {
          collab.insert("2", "b");
        },
      },
      ModifyLocalCollab {
        device_id: "3".to_string(),
        f: |collab| {
          collab.insert("3", "c");
        },
      },
      Wait { secs: 1 },
      AssertClientEqual {
        device_id_a: "1".to_string(),
        device_id_b: "2".to_string(),
      },
      AssertClientEqual {
        device_id_a: "2".to_string(),
        device_id_b: "3".to_string(),
      },
      AssertClientContent {
        device_id: "1".to_string(),
        expected: json!( {
          "1": "a",
          "2": "b",
          "3": "c",
          "map": {
            "task1": "a",
            "task2": "b"
          }
        }),
      },
    ])
    .await;
}

// One client offline, another client online, and then the offline client
// becomes online and receives the changes from the server.
#[tokio::test]
async fn one_online_and_another_client_offline_test() {
  let mut test = ScriptTest::new("1").await;
  test
    .run_scripts(vec![
      CreateClient {
        uid: 1,
        device_id: "1".to_string(),
      },
      // Make device 2 offline. It will try to sync with the server
      // when it becomes online.
      CreateEmptyClient {
        uid: 1,
        device_id: "2".to_string(),
      },
      DisconnectClient {
        device_id: "2".to_string(),
      },
      ModifyLocalCollab {
        device_id: "1".to_string(),
        f: |collab| {
          collab.insert("1", "a");
          collab.insert("2", "b");
        },
      },
      ConnectClient {
        device_id: "2".to_string(),
      },
      Wait {
        secs: DEFAULT_SYNC_TIMEOUT,
      },
      AssertClientEqual {
        device_id_a: "1".to_string(),
        device_id_b: "2".to_string(),
      },
    ])
    .await;
}

// Both the clients are offline, and then they become online and start syncing
// with the server.
#[tokio::test]
async fn two_clients_offline_test() {
  let mut test = ScriptTest::new("1").await;
  test
    .run_scripts(vec![
      CreateClient {
        uid: 1,
        device_id: "1".to_string(),
      },
      DisconnectClient {
        device_id: "1".to_string(),
      },
      CreateClient {
        uid: 1,
        device_id: "2".to_string(),
      },
      DisconnectClient {
        device_id: "2".to_string(),
      },
      ModifyLocalCollab {
        device_id: "1".to_string(),
        f: |collab| {
          collab.insert("1", "a");
          collab.insert("2", "b");
          collab.insert("3", "c");
        },
      },
      ModifyLocalCollab {
        device_id: "2".to_string(),
        f: |collab| {
          collab.insert("4", "d");
          collab.insert("5", "e");
          collab.insert("6", "f");
        },
      },
      ConnectClient {
        device_id: "1".to_string(),
      },
      ConnectClient {
        device_id: "2".to_string(),
      },
      Wait {
        secs: DEFAULT_SYNC_TIMEOUT * 2,
      },
      AssertClientEqual {
        device_id_a: "1".to_string(),
        device_id_b: "2".to_string(),
      },
    ])
    .await;
}

// Both the clients are offline, and then they become online and start syncing
// with the server.
#[tokio::test]
async fn two_clients_remove_vec_element_at_same_pos_test() {
  let mut test = ScriptTest::new("1").await;
  test
    .run_scripts(vec![
      CreateClient {
        uid: 1,
        device_id: "1".to_string(),
      },
      CreateClient {
        uid: 1,
        device_id: "2".to_string(),
      },
      ModifyLocalCollab {
        device_id: "1".to_string(),
        f: |collab| {
          collab.with_origin_transact_mut(|txn| {
            collab.create_array_with_txn(txn, "array", vec!["a"]);
          });
        },
      },
      Wait { secs: 1 },
      AssertClientEqual {
        device_id_a: "1".to_string(),
        device_id_b: "2".to_string(),
      },
      // Both clients remove the first element of the array and push back a new
      // element.
      ModifyLocalCollab {
        device_id: "1".to_string(),
        f: |collab| {
          collab.with_origin_transact_mut(|txn| {
            collab
              .get_array_with_txn(txn, vec!["array"])
              .unwrap()
              .remove(txn, 0);
            collab
              .get_array_with_txn(txn, vec!["array"])
              .unwrap()
              .push_back(txn, "aa".to_string());
          });
        },
      },
      Wait { secs: 1 },
      ModifyLocalCollab {
        device_id: "2".to_string(),
        f: |collab| {
          collab.with_origin_transact_mut(|txn| {
            collab
              .get_array_with_txn(txn, vec!["array"])
              .unwrap()
              .remove(txn, 0);
            collab
              .get_array_with_txn(txn, vec!["array"])
              .unwrap()
              .push_back(txn, "bb".to_string());
          });
        },
      },
      Wait { secs: 1 },
      AssertClientEqual {
        device_id_a: "1".to_string(),
        device_id_b: "2".to_string(),
      },
      AssertClientContent {
        device_id: "1".to_string(),
        // Last write wins
        expected: json!({
          "array": [
            "bb",
          ],
          "map": {
            "task1": "a",
            "task2": "b"
          }
        }),
      },
    ])
    .await;
}
