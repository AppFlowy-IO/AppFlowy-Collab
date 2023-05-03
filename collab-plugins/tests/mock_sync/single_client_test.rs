use collab_sync::client::sync::SYNC_TIMEOUT;
use serde_json::json;

use crate::util::ScriptTest;
use crate::util::TestScript::*;

#[tokio::test]
async fn single_write_test() {
  let mut test = ScriptTest::new(1, "1").await;
  // 1. add new client with device_id = 1
  // 2. modify collab with device_id = 1
  // 3. wait 1 second (for sync)
  // 4. assert client content
  // 5. assert client equal to server
  test
    .run_scripts(vec![
      CreateClient {
        uid: 1,
        device_id: "1".to_string(),
      },
      ModifyCollab {
        device_id: "1".to_string(),
        f: |collab| {
          collab.insert("1", "a");
        },
      },
      Wait { secs: 1 },
      AssertClientContent {
        device_id: "1".to_string(),
        expected: json!({
          "1": "a",
          "map": {
            "task1": "a",
            "task2": "b"
          }
        }),
      },
      AssertClientEqualToServer {
        device_id: "1".to_string(),
      },
    ])
    .await;
}

#[tokio::test]
async fn client_offline_test() {
  let mut test = ScriptTest::new(1, "1").await;
  // 1. add new client with device_id = 1
  // 2. set client offline
  // 3. modify collab with device_id = 1
  // 4. Check that the update is not sync with remote server
  test
    .run_scripts(vec![
      CreateClient {
        uid: 1,
        device_id: "1".to_string(),
      },
      DisconnectClient {
        device_id: "1".to_string(),
      },
      ModifyCollab {
        device_id: "1".to_string(),
        f: |collab| {
          collab.insert("1", "a");
        },
      },
      AssertClientContent {
        device_id: "1".to_string(),
        expected: json!({
          "1": "a",
          "map": {
            "task1": "a",
            "task2": "b"
          }
        }),
      },
      AssertServerContent {
        expected: json!({}),
      },
    ])
    .await;
}

#[tokio::test]
async fn client_offline_to_online_test() {
  let mut test = ScriptTest::new(1, "1").await;
  // 1. add new client with device_id = 1
  // 2. set client offline
  // 3. modify collab with device_id = 1
  // 4. set client online
  // 5. wait 1 second (for sync)
  // 6. check the server is sync or not
  test
    .run_scripts(vec![
      CreateClient {
        uid: 1,
        device_id: "1".to_string(),
      },
      DisconnectClient {
        device_id: "1".to_string(),
      },
      ModifyCollab {
        device_id: "1".to_string(),
        f: |collab| {
          collab.insert("1", "a");
        },
      },
      ConnectClient {
        device_id: "1".to_string(),
      },
      Wait { secs: 1 },
      AssertServerContent {
        expected: json!({
          "1": "a",
          "map": {
            "task1": "a",
            "task2": "b"
          }
        }),
      },
    ])
    .await;
}

#[tokio::test]
async fn client_multiple_write_test() {
  let mut test = ScriptTest::new(1, "1").await;
  test
    .run_scripts(vec![
      CreateClient {
        uid: 1,
        device_id: "1".to_string(),
      },
      ModifyCollab {
        device_id: "1".to_string(),
        f: |collab| {
          collab.insert("1", "a");
        },
      },
      ModifyCollab {
        device_id: "1".to_string(),
        f: |collab| {
          collab.insert("2", "b");
        },
      },
      ModifyCollab {
        device_id: "1".to_string(),
        f: |collab| {
          collab.insert("3", "c");
        },
      },
      Wait { secs: 1 },
      AssertServerContent {
        expected: json!({
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

#[tokio::test]
async fn client_unstable_network_write_test() {
  let mut test = ScriptTest::new(1, "1").await;
  let device_id = "1".to_string();
  test
    .run_scripts(vec![
      CreateClient {
        uid: 1,
        device_id: device_id.clone(),
      },
      ModifyCollab {
        device_id: device_id.clone(),
        f: |collab| {
          collab.insert("1", "a");
        },
      },
      // the server will be sync with the client within 1 second.
      Wait { secs: 1 },
      // 1. set client online after modify the document.
      DisconnectClient {
        device_id: device_id.clone(),
      },
      // 2. The server should not sync with the latest updates.
      ModifyCollab {
        device_id: device_id.clone(),
        f: |collab| {
          collab.insert("2", "b");
        },
      },
      ModifyCollab {
        device_id: "1".to_string(),
        f: |collab| {
          collab.insert("3", "c");
        },
      },
      Wait { secs: 1 },
      AssertServerContent {
        expected: json!({
          "1": "a",
          "map": {
            "task1": "a",
            "task2": "b"
          }
        }),
      },
      // 3. reconnect client
      ConnectClient {
        device_id: device_id.clone(),
      },
      // 4. wait SYNC_TIMEOUT second (for sync). After the timeout, the client will resent
      // the update to the server
      Wait {
        secs: SYNC_TIMEOUT * 2,
      },
      // 5. check the server is sync or not
      AssertServerContent {
        expected: json!({
          "1": "a",
          "2": "b",
          "3": "c",
          "map": {
            "task1": "a",
            "task2": "b"
          }
        }),
      },
      AssertClientEqualToServer {
        device_id: device_id.clone(),
      },
    ])
    .await;
}

#[tokio::test]
async fn client_reopen_test() {
  let mut test = ScriptTest::new(1, "1").await;
  let device_id = "1".to_string();
  test
    .run_scripts(vec![
      CreateClient {
        uid: 1,
        device_id: device_id.clone(),
      },
      DisconnectClient {
        device_id: device_id.clone(),
      },
      Wait { secs: 1 },
      AssertServerContent {
        expected: json!({}),
      },
    ])
    .await;

  let db = test.remove_client(&device_id).db;

  test
    .run_scripts(vec![
      AddClient {
        uid: 1,
        device_id: device_id.clone(),
        db,
      },
      // the server will be sync with the client within 1 second.
      Wait { secs: 1 },
      AssertServerContent {
        expected: json!(""),
      },
    ])
    .await;
}
