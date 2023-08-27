use crate::util::{spawn_client_with_empty_doc, spawn_server, wait_one_sec};
use serde_json::json;

#[tokio::test]
async fn send_single_update_to_server_test() {
  let _uid = 1;
  let object_id = "1";
  let server = spawn_server(object_id).await.unwrap();
  let client = spawn_client_with_empty_doc(object_id, server.address)
    .await
    .unwrap();

  // client -> sync step 1 -> server
  // client <- sync step 2 <- server
  wait_one_sec().await;
  // client -> update -> server
  // server apply update
  client.lock().insert("1", "a");
  wait_one_sec().await;

  let json1 = client.to_json_value();
  assert_json_diff::assert_json_eq!(
    json1,
    json!( {
      "1": "a"
    })
  );

  let json2 = server.get_doc_json(object_id);
  assert_eq!(json1, json2);
}

#[tokio::test]
async fn send_multiple_updates_to_server_test() {
  let _uid = 1;
  let object_id = "1";
  let server = spawn_server(object_id).await.unwrap();
  let client = spawn_client_with_empty_doc(object_id, server.address)
    .await
    .unwrap();
  wait_one_sec().await;
  {
    let client = client.lock();
    client.with_origin_transact_mut(|txn| {
      let map = client.insert_map_with_txn(txn, "map");
      map.insert_with_txn(txn, "task1", "a");
      map.insert_with_txn(txn, "task2", "b");
    });
  }
  wait_one_sec().await;
  {
    let client = client.lock();
    client.with_origin_transact_mut(|txn| {
      let map = client.get_map_with_txn(txn, vec!["map"]).unwrap();
      map.insert_with_txn(txn, "task3", "c");
    });
  }
  wait_one_sec().await;

  let json = server.get_doc_json(object_id);
  assert_json_diff::assert_json_eq!(
    json,
    json!( {
      "map": {
        "task1": "a",
        "task2": "b",
        "task3": "c"
      }
    })
  );
}

#[tokio::test]
async fn fetch_initial_state_from_server_test() {
  let _uid = 1;
  let object_id = "1";

  let server = spawn_server(object_id).await.unwrap();
  server.mut_groups(object_id, |collab| {
    collab.insert("1", "a");
  });

  let client = spawn_client_with_empty_doc(object_id, server.address)
    .await
    .unwrap();
  wait_one_sec().await;

  let json = client.to_json_value();
  assert_json_diff::assert_json_eq!(
    json,
    json!( {
      "1": "a"
    })
  );
}

#[tokio::test]
async fn send_local_doc_initial_state_to_server() {
  let _uid = 1;
  let object_id = "1";

  let server = spawn_server(object_id).await.unwrap();
  let client = spawn_client_with_empty_doc(object_id, server.address)
    .await
    .unwrap();
  wait_one_sec().await;
  {
    let client = client.lock();
    client.with_origin_transact_mut(|txn| {
      let map = client.insert_map_with_txn(txn, "map");
      map.insert_with_txn(txn, "task1", "a");
      map.insert_with_txn(txn, "task2", "b");
    });
  }
  wait_one_sec().await;
  let json = server.get_doc_json(object_id);
  assert_json_diff::assert_json_eq!(
    json,
    json!( {
      "map": {
        "task1": "a",
        "task2": "b"
      }
    })
  );
}

#[tokio::test]
async fn send_local_doc_initial_state_to_server_multiple_times() {
  let _uid = 1;
  let object_id = "1";

  let server = spawn_server(object_id).await.unwrap();
  let client = spawn_client_with_empty_doc(object_id, server.address)
    .await
    .unwrap();
  wait_one_sec().await;
  {
    let client = client.lock();
    client.with_origin_transact_mut(|txn| {
      let map = client.insert_map_with_txn(txn, "map");
      map.insert_with_txn(txn, "task1", "a");
      map.insert_with_txn(txn, "task2", "b");
    });
  }
  wait_one_sec().await;

  let remote_doc_json = server.get_doc_json(object_id);

  for _i in 0..3 {
    let _client = spawn_client_with_empty_doc(object_id, server.address)
      .await
      .unwrap();
    wait_one_sec().await;
    assert_eq!(remote_doc_json, server.get_doc_json(object_id));
  }
}
