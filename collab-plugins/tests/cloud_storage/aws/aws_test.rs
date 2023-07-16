use std::sync::Arc;
use std::time::Duration;

use collab_plugins::cloud_storage::aws::is_enable_aws_dynamodb;
use serde_json::{json, Map, Value};
use tokio::sync::RwLock;

use dotenv::dotenv;
use nanoid::nanoid;

use crate::cloud_storage::aws::script::TestScript::*;
use crate::cloud_storage::aws::script::{make_id, AWSStorageTest};
use crate::util::generate_random_string;

/// ⚠️run this test, it will alter the remote table
#[tokio::test]
async fn collab_with_aws_plugin_test() {
  dotenv().ok();
  if !is_enable_aws_dynamodb().await {
    return;
  }
  let object_id = nanoid!(5);
  let uid = 1;
  let mut test = AWSStorageTest::new(uid);
  println!("object_id: {}", object_id);
  test
    .run_scripts(vec![
      CreateCollab {
        uid,
        object_id: object_id.clone(),
        sync_per_secs: 1,
      },
      ModifyCollab {
        uid,
        object_id: object_id.clone(),
        f: Box::new(|collab| {
          collab.insert("123", "abc");
        }),
      },
      ModifyCollab {
        uid,
        object_id: object_id.clone(),
        f: Box::new(|collab| {
          collab.insert("456", "efg");
        }),
      },
      ModifyCollab {
        uid,
        object_id: object_id.clone(),
        f: Box::new(|collab| {
          collab.insert("789", "hij");
        }),
      },
      Wait { secs: 6 },
      AssertLocal {
        uid,
        object_id: object_id.clone(),
        expected: json!( {
          "123": "abc",
          "456": "efg",
          "789": "hij",
        }),
      },
      AssertRemote {
        object_id: object_id.clone(),
        expected: json!( {
          "123": "abc",
          "456": "efg",
          "789": "hij",
        }),
      },
    ])
    .await;
}

/// ⚠️run this test, it will alter the remote table
#[tokio::test]
async fn single_client_edit_aws_doc_10_times_test() {
  dotenv().ok();
  if !is_enable_aws_dynamodb().await {
    return;
  }
  let object_id = nanoid!(5);
  let uid = 1;
  let mut test = AWSStorageTest::new(uid);
  test
    .run_scripts(vec![CreateCollab {
      uid,
      object_id: object_id.clone(),
      sync_per_secs: 1,
    }])
    .await;
  let mut map = Map::new();
  for i in 0..10 {
    let key = i.to_string();
    let value = generate_random_string(10);
    map.insert(key.clone(), Value::String(value.clone()));
    test
      .run_scripts(vec![ModifyCollab {
        uid,
        object_id: object_id.clone(),
        f: Box::new(move |collab| {
          collab.insert(&key, value);
        }),
      }])
      .await;
  }
  tokio::time::sleep(Duration::from_secs(3)).await;
  test
    .run_scripts(vec![
      AssertLocal {
        uid,
        object_id: object_id.clone(),
        expected: Value::Object(map.clone()),
      },
      AssertRemote {
        object_id: object_id.clone(),
        expected: Value::Object(map),
      },
    ])
    .await;
}

/// ⚠️run this test, it will alter the remote table
// Two clients edit the same doc
#[tokio::test]
async fn multi_clients_edit_aws_doc_10_times_test() {
  dotenv().ok();
  if !is_enable_aws_dynamodb().await {
    return;
  }
  let object_id = nanoid!(5);
  let uid = 1;
  let test = Arc::new(RwLock::new(AWSStorageTest::new(uid)));
  test
    .write()
    .await
    .run_scripts(vec![
      CreateCollab {
        uid,
        object_id: object_id.clone(),
        sync_per_secs: 1,
      },
      CreateCollab {
        uid: 2,
        object_id: object_id.clone(),
        sync_per_secs: 1,
      },
    ])
    .await;

  let mut map = Map::new();
  let mut map_1 = Map::new();
  let mut map_2 = Map::new();
  for uid in 1..3 {
    for i in 0..10 {
      let key = format!("{}-{}", uid, i);
      let value = generate_random_string(10);
      map.insert(key.clone(), Value::String(value.clone()));
      if uid == 1 {
        map_1.insert(key.clone(), Value::String(value.clone()));
      } else {
        map_2.insert(key.clone(), Value::String(value.clone()));
      }

      let cloned_test = test.clone();
      let cloned_object_id = object_id.clone();
      tokio::spawn(async move {
        let collab = cloned_test
          .write()
          .await
          .collab_by_id
          .get(&make_id(uid, &cloned_object_id))
          .unwrap()
          .clone();
        collab.lock().insert(&key, value);
      });
    }
  }

  tokio::time::sleep(Duration::from_secs(3)).await;
  test
    .write()
    .await
    .run_scripts(vec![
      AssertLocal {
        uid: 1,
        object_id: object_id.clone(),
        expected: Value::Object(map_1.clone()),
      },
      AssertLocal {
        uid: 2,
        object_id: object_id.clone(),
        expected: Value::Object(map_2.clone()),
      },
      AssertRemote {
        object_id: object_id.clone(),
        expected: Value::Object(map),
      },
    ])
    .await;
}
