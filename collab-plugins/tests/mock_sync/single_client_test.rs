use collab_sync::client::sync::DEFAULT_SYNC_TIMEOUT;
use serde_json::json;
use yrs::Array;

use crate::util::TestScript::*;
use crate::util::{create_db, Rng, ScriptTest};

#[tokio::test]
async fn single_write_test() {
  let mut test = ScriptTest::new("1").await;
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
      ModifyLocalCollab {
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
  let mut test = ScriptTest::new("1").await;
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
      ModifyLocalCollab {
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
  let mut test = ScriptTest::new("1").await;
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
      ModifyLocalCollab {
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
  let mut test = ScriptTest::new("1").await;
  test
    .run_scripts(vec![
      CreateClient {
        uid: 1,
        device_id: "1".to_string(),
      },
      ModifyLocalCollab {
        device_id: "1".to_string(),
        f: |collab| {
          collab.insert("1", "a");
        },
      },
      ModifyLocalCollab {
        device_id: "1".to_string(),
        f: |collab| {
          collab.insert("2", "b");
        },
      },
      ModifyLocalCollab {
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
  let mut test = ScriptTest::new("1").await;
  let device_id = "1".to_string();
  test
    .run_scripts(vec![
      CreateClient {
        uid: 1,
        device_id: device_id.clone(),
      },
      ModifyLocalCollab {
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
      ModifyLocalCollab {
        device_id: device_id.clone(),
        f: |collab| {
          collab.insert("2", "b");
        },
      },
      ModifyLocalCollab {
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
        secs: DEFAULT_SYNC_TIMEOUT * 2,
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

// The version of the local document is a ahead of the remote document. So the server need
// to get the update from the client.
#[tokio::test]
async fn server_sync_state_vector_with_client_test() {
  let mut test = ScriptTest::new("1").await;
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
      // The server does not sync with the client
      AssertServerContent {
        expected: json!({}),
      },
    ])
    .await;

  // Open the document with build-in data
  let db = test.remove_client(&device_id).db;
  test
    .run_scripts(vec![
      CreateClientWithDb {
        uid: 1,
        device_id: device_id.clone(),
        db,
      },
      AssertClientContent {
        device_id: device_id.clone(),
        expected: json!({
          "map": {
            "task1": "a",
            "task2": "b"
          }
        }),
      },
      // the server will be sync with the client within 1 second.
      Wait { secs: 1 },
      AssertServerContent {
        expected: json!({
          "map": {
            "task1": "a",
            "task2": "b"
          }
        }),
      },
    ])
    .await;
}

// The version of the local document is a ahead of the remote document. The server will sync with
// the the same client multiple times. Check out the content is same as the local document.
#[tokio::test]
async fn server_sync_state_vector_multiple_time_with_client_test() {
  let mut test = ScriptTest::new("1").await;
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
    ])
    .await;

  // Open the document with build-in data
  let db = test.remove_client(&device_id).db;
  for _ in 0..5 {
    test
      .run_scripts(vec![
        CreateClientWithDb {
          uid: 1,
          device_id: device_id.clone(),
          db: db.clone(),
        },
        Wait { secs: 1 },
        AssertServerContent {
          expected: json!({
            "map": {
              "task1": "a",
              "task2": "b"
            }
          }),
        },
      ])
      .await;
  }
}

// Both the clients are offline, and then they become online and start syncing
// with the server.
#[tokio::test]
async fn client_periodically_open_doc_test() {
  let mut test = ScriptTest::new("1").await;
  let db = create_db();
  test
    .run_scripts(vec![
      CreateClientWithDb {
        uid: 1,
        device_id: "1".to_string(),
        db: db.clone(),
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
      AssertClientEqualToServer {
        device_id: "1".to_string(),
      },
      DisconnectClient {
        device_id: "1".to_string(),
      },
      ModifyLocalCollab {
        device_id: "1".to_string(),
        f: |collab| {
          collab.with_origin_transact_mut(|txn| {
            let array = collab.get_array_with_txn(txn, vec!["array"]).unwrap();
            array.push_back(txn, "b");
            array.push_back(txn, "c");
          });
        },
      },
      Wait { secs: 1 },
      AssertServerContent {
        expected: json!({
          "array": [
            "a",
          ]
        }),
      },
    ])
    .await;

  test
    .run_scripts(vec![
      CreateClientWithDb {
        uid: 1,
        device_id: "1".to_string(),
        db: db.clone(),
      },
      Wait { secs: 1 },
      AssertClientEqualToServer {
        device_id: "1".to_string(),
      },
      AssertServerContent {
        expected: json!({
          "array": [
            "a",
            "b",
            "c"
          ]
        }),
      },
      DisconnectClient {
        device_id: "1".to_string(),
      },
    ])
    .await;

  test
    .run_scripts(vec![
      CreateClientWithDb {
        uid: 1,
        device_id: "1".to_string(),
        db: db.clone(),
      },
      Wait { secs: 1 },
      ModifyLocalCollab {
        device_id: "1".to_string(),
        f: |collab| {
          collab.with_origin_transact_mut(|txn| {
            let array = collab.get_array_with_txn(txn, vec!["array"]).unwrap();
            array.push_back(txn, "d");
            array.push_back(txn, "e");
          });
        },
      },
      Wait { secs: 1 },
      AssertServerContent {
        expected: json!({
          "array": [
            "a",
            "b",
            "c",
            "d",
            "e"
          ]
        }),
      },
    ])
    .await
}

// Edit the document with the same client multiple times by inserting a random string to the array.
#[tokio::test]
async fn client_periodically_edit_test() {
  let mut test = ScriptTest::new("1").await;
  let db = create_db();
  test
    .run_scripts(vec![
      CreateClientWithDb {
        uid: 1,
        device_id: "1".to_string(),
        db: db.clone(),
      },
      ModifyLocalCollab {
        device_id: "1".to_string(),
        f: |collab| {
          collab.with_origin_transact_mut(|txn| {
            collab.create_array_with_txn::<String>(txn, "array", vec![]);
          });
        },
      },
    ])
    .await;

  let mut rng = Rng::default();
  #[derive(serde::Serialize)]
  struct MyArray {
    array: Vec<String>,
  }
  let mut array = MyArray { array: vec![] };
  for _ in 0..100 {
    let s = rng.gen_string(1);
    array.array.push(s.clone());

    let client = test.clients.get_mut("1").unwrap();
    let collab = client.lock();
    collab.with_origin_transact_mut(|txn| {
      collab
        .get_array_with_txn(txn, vec!["array"])
        .unwrap()
        .push_back(txn, s);
    });
  }

  test
    .run_scripts(vec![
      Wait { secs: 2 },
      AssertServerContent {
        expected: serde_json::to_value(&array).unwrap(),
      },
    ])
    .await;
}

// The version of the local document is less than the remote document. So the client need to
// sync with the remote document.
#[tokio::test]
async fn client_sync_with_server() {
  let mut test = ScriptTest::new("1").await;
  let device_id = "1".to_string();
  test
    .run_scripts(vec![
      ModifyRemoteCollab {
        f: |collab| {
          collab.insert("title", "hello world");
        },
      },
      CreateClient {
        uid: 1,
        device_id: device_id.clone(),
      },
      Wait { secs: 1 },
      AssertServerContent {
        expected: json!({
          "map": {
            "task1": "a",
            "task2": "b"
          },
          "title": "hello world"
        }),
      },
    ])
    .await;
}

// The version of the local document is less than the remote document. So the client need to
// sync with the remote document.
#[tokio::test]
async fn client_continuously_sync_with_server() {
  let mut test = ScriptTest::new("1").await;
  let device_id = "1".to_string();
  test
    .run_scripts(vec![
      ModifyRemoteCollab {
        f: |collab| {
          collab.insert("title", "hello world");
        },
      },
      CreateClient {
        uid: 1,
        device_id: device_id.clone(),
      },
      ModifyRemoteCollab {
        f: |collab| {
          collab.insert("name", "appflowy");
        },
      },
      ModifyRemoteCollab {
        f: |collab| {
          collab.insert("desc", "open source project");
        },
      },
      Wait { secs: 1 },
      AssertServerContent {
        expected: json!({
          "desc": "open source project",
          "map": {
            "task1": "a",
            "task2": "b"
          },
          "name": "appflowy",
          "title": "hello world"
        }),
      },
    ])
    .await;
}

#[tokio::test]
async fn server_state_vector_size_test() {
  let mut test = ScriptTest::new("1").await;
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
    ])
    .await;

  let db = test.remove_client(&device_id).db;

  // Open the document with build-in data
  let client = CreateClientWithDb {
    uid: 1,
    device_id: device_id.clone(),
    db: db.clone(),
  };
  test.run_scripts(vec![client, Wait { secs: 1 }]).await;

  // Open the document with build-in data
  let client = CreateClientWithDb {
    uid: 1,
    device_id: device_id.clone(),
    db: db.clone(),
  };
  let assert_server_content = AssertServerContent {
    expected: json!({
      "map": {
        "task1": "a",
        "task2": "b"
      }
    }),
  };
  test
    .run_scripts(vec![client, Wait { secs: 1 }, assert_server_content])
    .await;
}

/// Test if the server is save the document to the database correctly.
#[tokio::test]
async fn rerun_server_test() {
  let mut test = ScriptTest::new("1").await;
  test
    .run_scripts(vec![
      CreateClient {
        uid: 1,
        device_id: 1.to_string(),
      },
      Wait { secs: 1 },
      AssertServerContent {
        expected: json!({
          "map": {
            "task1": "a",
            "task2": "b"
          },
        }),
      },
      DisconnectClient {
        device_id: 1.to_string(),
      },
      RerunServer,
      AssertServerContent {
        expected: json!({
          "map": {
            "task1": "a",
            "task2": "b"
          },
        }),
      },
      // Sync the content to device_id:2 after the server rerun.
      CreateClient {
        uid: 1,
        device_id: 2.to_string(),
      },
      Wait { secs: 1 },
      AssertClientEqualToServer {
        device_id: 2.to_string(),
      },
    ])
    .await;
}
