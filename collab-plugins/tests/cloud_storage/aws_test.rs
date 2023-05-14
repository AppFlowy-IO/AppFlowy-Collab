use crate::cloud_storage::script::CloudStorageTest;
use crate::cloud_storage::script::TestScript::{
  AssertLocal, AssertRemote, CreateCollab, ModifyCollab, Wait,
};

use nanoid::nanoid;
use serde_json::json;
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
      ModifyCollab {
        uid: 1,
        object_id: object_id.clone(),
        f: |collab| {
          collab.insert("456", "efg");
        },
      },
      ModifyCollab {
        uid: 1,
        object_id: object_id.clone(),
        f: |collab| {
          collab.insert("789", "hij");
        },
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
  let object_id = nanoid!(5);
  let mut test = CloudStorageTest::new();
  test
    .run_scripts(vec![CreateCollab {
      uid: 1,
      object_id: object_id.clone(),
    }])
    .await;
  tracing::trace!("object_id: {}", object_id);
  for _ in 0..10 {
    test
      .run_scripts(vec![ModifyCollab {
        uid: 1,
        object_id: object_id.clone(),
        f: move |collab| {
          collab.insert("a", "b");
        },
      }])
      .await;
  }
  tokio::time::sleep(Duration::from_secs(5)).await;
}
