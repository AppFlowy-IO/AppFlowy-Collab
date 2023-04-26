use std::time::Duration;

use serde_json::json;

use crate::util::{make_test_client, spawn_server};

#[tokio::test]
async fn sync_test() {
  let server = spawn_server().await.unwrap();
  let client = make_test_client(1, "1", server.address).await.unwrap();
  tokio::time::sleep(Duration::from_secs(1)).await;

  client.lock().collab.insert("1", "a");

  tokio::time::sleep(Duration::from_secs(1)).await;

  let json1 = client.lock().collab.to_json_value();
  let json2 = server
    .groups
    .get("1")
    .unwrap()
    .awareness
    .lock()
    .collab
    .to_json_value();

  assert_json_diff::assert_json_eq!(
    json1,
    json!( {
      "1": "a"
    })
  );
  assert_eq!(json1, json2);
}
