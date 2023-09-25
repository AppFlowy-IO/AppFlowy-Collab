use crate::sync::single_client_test::{test_sync_object, test_sync_object_pair};
use crate::util::{spawn_client, spawn_server, wait_one_sec};
use serde_json::json;

#[tokio::test]
async fn open_existing_doc_with_different_client_test() {
  let uid = 1;
  let object = test_sync_object();
  let server = spawn_server(object.clone()).await.unwrap();
  let (_, _client_1) = spawn_client(uid, object.clone(), server.address)
    .await
    .unwrap();
  let (_, _client_2) = spawn_client(uid, object.clone(), server.address)
    .await
    .unwrap();
  wait_one_sec().await;

  assert_json_diff::assert_json_eq!(
    server.get_doc_json(&object.object_id),
    json!( {
      "map": {
        "task1": "a",
        "task2": "b"
      }
    })
  );
}

#[tokio::test]
async fn single_write_sync_with_server_test() {
  let uid = 1;
  let (object_1, object_2) = test_sync_object_pair();

  let server = spawn_server(object_1.clone()).await.unwrap();
  let (_, client_1) = spawn_client(uid, object_1, server.address).await.unwrap();
  let (_, client_2) = spawn_client(uid, object_2, server.address).await.unwrap();
  wait_one_sec().await;
  {
    let client = client_1.lock();
    client.with_origin_transact_mut(|txn| {
      let map = client.get_map_with_txn(txn, vec!["map"]).unwrap();
      map.insert_with_txn(txn, "task3", "c");
      map.insert_with_txn(txn, "task4", "d");
    });
  }
  wait_one_sec().await;
  assert_json_diff::assert_json_eq!(
    client_1.to_json_value(),
    json!( {
      "map": {
        "task1": "a",
        "task2": "b",
        "task3": "c",
        "task4": "d",
      }
    })
  );
  assert_eq!(client_1.to_json_value(), client_2.to_json_value());
}

// Different clients write to the same document
#[tokio::test]
async fn two_writers_test() {
  let uid = 1;
  let object = test_sync_object();
  let server = spawn_server(object.clone()).await.unwrap();
  let (_, client_1) = spawn_client(uid, object.clone(), server.address)
    .await
    .unwrap();
  let (_, client_2) = spawn_client(uid, object.clone(), server.address)
    .await
    .unwrap();
  wait_one_sec().await;

  {
    let client = client_1.lock();
    client.with_origin_transact_mut(|txn| {
      let map = client.get_map_with_txn(txn, vec!["map"]).unwrap();
      map.insert_with_txn(txn, "task3", "c");
    });
  }
  {
    let client = client_2.lock();
    client.with_origin_transact_mut(|txn| {
      let map = client.get_map_with_txn(txn, vec!["map"]).unwrap();
      map.insert_with_txn(txn, "task4", "d");
    });
  }

  wait_one_sec().await;

  let client_1_json = client_1.to_json_value();
  let client_2_json = client_2.to_json_value();
  let server_json = server.get_doc_json(&object.object_id);

  assert_eq!(client_1_json, client_2_json);
  assert_eq!(client_1_json, server_json);
  assert_json_diff::assert_json_eq!(
    client_1_json,
    json!( {
      "map": {
        "task1": "a",
        "task2": "b",
        "task3": "c",
        "task4": "d"
      }
    })
  );
}

#[tokio::test]
async fn two_clients_last_write_win_test() {
  let uid = 1;
  let object = test_sync_object();
  let server = spawn_server(object.clone()).await.unwrap();
  let (_, client_1) = spawn_client(uid, object.clone(), server.address)
    .await
    .unwrap();
  let (_, client_2) = spawn_client(uid, object.clone(), server.address)
    .await
    .unwrap();
  wait_one_sec().await;
  {
    let client = client_1.lock();
    client.with_origin_transact_mut(|txn| {
      let map = client.get_map_with_txn(txn, vec!["map"]).unwrap();
      map.insert_with_txn(txn, "task2", "bb");
    });
  }
  wait_one_sec().await;
  {
    let client = client_2.lock();
    client.with_origin_transact_mut(|txn| {
      let map = client.get_map_with_txn(txn, vec!["map"]).unwrap();
      map.insert_with_txn(txn, "task2", "bbb");
    });
  }

  wait_one_sec().await;
  let client_1_json = client_1.to_json_value();
  let client_2_json = client_2.to_json_value();
  let server_json = server.get_doc_json(&object.object_id);
  assert_eq!(client_1_json, client_2_json);
  assert_eq!(client_1_json, server_json);
  assert_json_diff::assert_json_eq!(
    client_1_json,
    json!( {
      "map": {
        "task1": "a",
        "task2": "bbb"
      }
    })
  );
}

#[tokio::test]
async fn last_write_win_test() {
  let uid = 1;
  let object = test_sync_object();
  let server = spawn_server(object.clone()).await.unwrap();
  let (_, client_1) = spawn_client(uid, object.clone(), server.address)
    .await
    .unwrap();
  let (_, client_2) = spawn_client(uid, object.clone(), server.address)
    .await
    .unwrap();
  wait_one_sec().await;
  {
    let client = client_1.lock();
    client.with_origin_transact_mut(|txn| {
      let map = client.get_map_with_txn(txn, vec!["map"]).unwrap();
      map.insert_with_txn(txn, "task2", "bb");
    });
  }
  wait_one_sec().await;
  {
    let client = client_2.lock();
    client.with_origin_transact_mut(|txn| {
      let map = client.get_map_with_txn(txn, vec!["map"]).unwrap();
      map.insert_with_txn(txn, "task2", "bbb");
    });
  }

  wait_one_sec().await;
  let client_1_json = client_1.to_json_value();
  let client_2_json = client_2.to_json_value();
  let server_json = server.get_doc_json(&object.object_id);
  assert_eq!(client_1_json, client_2_json);
  assert_eq!(client_1_json, server_json);
  assert_json_diff::assert_json_eq!(
    client_1_json,
    json!( {
      "map": {
        "task1": "a",
        "task2": "bbb"
      }
    })
  );
}
