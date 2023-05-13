use crate::cloud_storage::script::CloudStorageTest;
use crate::cloud_storage::script::TestScript::{
  AssertLocal, AssertRemote, CreateCollab, ModifyCollab, RemoveCollab, Wait,
};
use crate::cloud_storage::util::is_enable_aws_test;
use crate::setup_log;
use crate::util::wait_five_sec;
use collab::core::collab::MutexCollab;
use collab::core::origin::{CollabClient, CollabOrigin};
use collab_plugins::cloud_storage_plugin::AWSDynamoDBPlugin;
use nanoid::nanoid;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;

#[tokio::test]
async fn collab_with_aws_plugin_test() {
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
        f: |collab| {
          collab.insert("123", "abc");
        },
      },
      Wait { secs: 1 },
      ModifyCollab {
        uid: 1,
        object_id: object_id.clone(),
        f: |collab| {
          collab.insert("456", "efg");
        },
      },
      Wait { secs: 1 },
      ModifyCollab {
        uid: 1,
        object_id: object_id.clone(),
        f: |collab| {
          collab.insert("789", "hij");
        },
      },
      Wait { secs: 1 },
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
  let object_id = nanoid!(5);
  let mut test = CloudStorageTest::new();
  test
    .run_scripts(vec![CreateCollab {
      uid: 1,
      object_id: object_id.clone(),
    }])
    .await;
  tracing::trace!("object_id: {}", object_id);
  for i in 0..10 {
    test
      .run_scripts(vec![ModifyCollab {
        uid: 1,
        object_id: object_id.clone(),
        f: move |collab| {
          // collab.insert(&format!("a{}", i), i);
          collab.insert("a", "b");
        },
      }])
      .await;
  }
  tokio::time::sleep(Duration::from_secs(5)).await;
}
