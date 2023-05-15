use crate::cloud_storage::script::CloudStorageTest;
use crate::cloud_storage::script::TestScript::{
  AssertLocal, AssertRemote, CreateCollab, ModifyCollab, Wait,
};

use crate::cloud_storage::util::{generate_random_string, is_enable_aws_test};
use nanoid::nanoid;

use serde_json::{json, Map, Value};
use std::time::Duration;

#[tokio::test]
async fn collab_with_aws_plugin_test() {
  if !is_enable_aws_test().await {
    return;
  }
  let object_id = nanoid!(5);
  let mut test = CloudStorageTest::new();
  println!("object_id: {}", object_id);
  test
    .run_scripts(vec![
      CreateCollab {
        uid: 1,
        object_id: object_id.clone(),
      },
      ModifyCollab {
        uid: 1,
        object_id: object_id.clone(),
        f: Box::new(|collab| {
          collab.insert("123", "abc");
        }),
      },
      ModifyCollab {
        uid: 1,
        object_id: object_id.clone(),
        f: Box::new(|collab| {
          collab.insert("456", "efg");
        }),
      },
      ModifyCollab {
        uid: 1,
        object_id: object_id.clone(),
        f: Box::new(|collab| {
          collab.insert("789", "hij");
        }),
      },
      Wait { secs: 6 },
      AssertLocal {
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

#[tokio::test]
async fn edit_aws_doc_10_times_test() {
  if !is_enable_aws_test().await {
    return;
  }
  let object_id = nanoid!(5);
  let mut test = CloudStorageTest::new();
  test
    .run_scripts(vec![CreateCollab {
      uid: 1,
      object_id: object_id.clone(),
    }])
    .await;
  let mut map = Map::new();
  for i in 0..10 {
    let key = i.to_string();
    let value = generate_random_string(10);
    map.insert(key.clone(), Value::String(value.clone()));
    test
      .run_scripts(vec![ModifyCollab {
        uid: 1,
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
