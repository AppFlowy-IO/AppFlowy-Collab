use collab_plugins::sync_plugin::client::DEFAULT_SYNC_TIMEOUT;
use serde_json::json;
use yrs::Array;

use crate::util::ScriptTest;
use crate::util::TestScript::*;

#[tokio::test]
async fn three_clients_on_line_test() {
  let object_id = uuid::Uuid::new_v4().to_string();
  let mut test = ScriptTest::new(&object_id).await;
  let device_id_1 = uuid::Uuid::new_v4().to_string();
  let device_id_2 = uuid::Uuid::new_v4().to_string();
  let device_id_3 = uuid::Uuid::new_v4().to_string();
  test
    .run_scripts(vec![
      CreateClient {
        uid: 1,
        device_id: device_id_1.clone(),
      },
      CreateEmptyClient {
        uid: 1,
        device_id: device_id_2.clone(),
      },
      CreateEmptyClient {
        uid: 1,
        device_id: device_id_3.clone(),
      },
      ModifyLocalCollab {
        device_id: device_id_1.clone(),
        f: |collab| {
          collab.insert("1", "a");
        },
      },
      ModifyLocalCollab {
        device_id: device_id_2.clone(),
        f: |collab| {
          collab.insert("2", "b");
        },
      },
      ModifyLocalCollab {
        device_id: device_id_3.clone(),
        f: |collab| {
          collab.insert("3", "c");
        },
      },
      Wait { secs: 1 },
      AssertClientEqual {
        device_id_a: device_id_1.clone(),
        device_id_b: device_id_2.clone(),
      },
      AssertClientEqual {
        device_id_a: device_id_2.clone(),
        device_id_b: device_id_3.clone(),
      },
      AssertClientContent {
        device_id: device_id_1.clone(),
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
  let object_id = uuid::Uuid::new_v4().to_string();
  let device_id_1 = uuid::Uuid::new_v4().to_string();
  let device_id_2 = uuid::Uuid::new_v4().to_string();
  let mut test = ScriptTest::new(&object_id).await;
  test
    .run_scripts(vec![
      CreateClient {
        uid: 1,
        device_id: device_id_1.clone(),
      },
      // Make device 2 offline. It will try to sync with the server
      // when it becomes online.
      CreateEmptyClient {
        uid: 1,
        device_id: device_id_2.clone(),
      },
      DisconnectClient {
        device_id: device_id_2.clone(),
      },
      ModifyLocalCollab {
        device_id: device_id_1.clone(),
        f: |collab| {
          collab.insert("1", "a");
          collab.insert("2", "b");
        },
      },
      ConnectClient {
        device_id: device_id_2.clone(),
      },
      Wait {
        secs: DEFAULT_SYNC_TIMEOUT,
      },
      AssertClientEqual {
        device_id_a: device_id_1,
        device_id_b: device_id_2,
      },
    ])
    .await;
}

// Both the clients are offline, and then they become online and start syncing
// with the server.
#[tokio::test]
async fn two_clients_offline_test() {
  let object_id = uuid::Uuid::new_v4().to_string();
  let device_id_1 = uuid::Uuid::new_v4().to_string();
  let device_id_2 = uuid::Uuid::new_v4().to_string();
  let mut test = ScriptTest::new(&object_id).await;
  test
    .run_scripts(vec![
      CreateClient {
        uid: 1,
        device_id: device_id_1.clone(),
      },
      DisconnectClient {
        device_id: device_id_1.clone(),
      },
      CreateClient {
        uid: 1,
        device_id: device_id_2.clone(),
      },
      DisconnectClient {
        device_id: device_id_2.clone(),
      },
      ModifyLocalCollab {
        device_id: device_id_1.clone(),
        f: |collab| {
          collab.insert("1", "a");
          collab.insert("2", "b");
          collab.insert("3", "c");
        },
      },
      ModifyLocalCollab {
        device_id: device_id_2.clone(),
        f: |collab| {
          collab.insert("4", "d");
          collab.insert("5", "e");
          collab.insert("6", "f");
        },
      },
      ConnectClient {
        device_id: device_id_1.clone(),
      },
      ConnectClient {
        device_id: device_id_2.clone(),
      },
      Wait {
        secs: DEFAULT_SYNC_TIMEOUT * 2,
      },
      AssertClientEqual {
        device_id_a: device_id_1,
        device_id_b: device_id_2,
      },
    ])
    .await;
}

// Both the clients are offline, and then they become online and start syncing
// with the server.
#[tokio::test]
async fn two_clients_remove_vec_element_at_same_pos_test() {
  let object_id = uuid::Uuid::new_v4().to_string();
  let device_id_1 = uuid::Uuid::new_v4().to_string();
  let device_id_2 = uuid::Uuid::new_v4().to_string();
  let mut test = ScriptTest::new(&object_id).await;
  test
    .run_scripts(vec![
      CreateClient {
        uid: 1,
        device_id: device_id_1.clone(),
      },
      CreateClient {
        uid: 1,
        device_id: device_id_2.clone(),
      },
      ModifyLocalCollab {
        device_id: device_id_1.clone(),
        f: |collab| {
          collab.with_origin_transact_mut(|txn| {
            collab.create_array_with_txn(txn, "array", vec!["a"]);
          });
        },
      },
      Wait { secs: 1 },
      AssertClientEqual {
        device_id_a: device_id_1.clone(),
        device_id_b: device_id_2.clone(),
      },
      // Both clients remove the first element of the array and push back a new
      // element.
      ModifyLocalCollab {
        device_id: device_id_1.clone(),
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
        device_id: device_id_2.clone(),
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
        device_id_a: device_id_1.clone(),
        device_id_b: device_id_2.clone(),
      },
      AssertClientContent {
        device_id: device_id_1.clone(),
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
