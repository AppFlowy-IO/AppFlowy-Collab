use collab_plugins::local_storage::CollabPersistenceConfig;
use rand::Rng;
use serde_json::json;

use crate::disk::script::CollabPersistenceTest;
use crate::disk::script::Script::*;

#[tokio::test]
async fn disable_snapshot_test() {
  let mut test = CollabPersistenceTest::new(CollabPersistenceConfig::new().enable_snapshot(false));
  let doc_id = "1".to_string();
  test
    .run_scripts(vec![OpenDocument { id: doc_id.clone() }])
    .await;

  for i in 1..=20 {
    test
      .run_script(InsertKeyValue {
        id: doc_id.clone(),
        key: i.to_string(),
        value: i.into(),
      })
      .await;
  }

  test
    .run_scripts(vec![
      AssertNumOfUpdates {
        id: doc_id.clone(),
        expected: 20,
      },
      AssertNumOfSnapshots {
        id: doc_id,
        expected: 0,
      },
    ])
    .await;
}

#[tokio::test]
async fn reopen_doc_snapshot_test() {
  let mut test = CollabPersistenceTest::new(
    CollabPersistenceConfig::new()
      .enable_snapshot(true)
      .snapshot_per_update(9),
  );
  let doc_id = "1".to_string();
  test
    .run_scripts(vec![OpenDocument { id: doc_id.clone() }])
    .await;
  for i in 1..=9 {
    test
      .run_script(InsertKeyValue {
        id: doc_id.clone(),
        key: i.to_string(),
        value: i.into(),
      })
      .await;
  }
  test
    .run_scripts(vec![
      AssertNumOfUpdates {
        id: doc_id.clone(),
        expected: 9,
      },
      // wait for snapshot to write to disk for 1 second
      Wait(1),
      AssertNumOfSnapshots {
        id: doc_id.clone(),
        expected: 1,
      },
      CloseDocument { id: doc_id.clone() },
    ])
    .await;

  // reopen
  test
    .run_scripts(vec![OpenDocument { id: doc_id.clone() }])
    .await;
  test
    .run_scripts(vec![
      AssertNumOfUpdates {
        id: doc_id.clone(),
        expected: 9,
      },
      AssertNumOfSnapshots {
        id: doc_id.clone(),
        expected: 1,
      },
      AssertDocument {
        id: doc_id.clone(),
        expected: json!({
          "1": 1.0,
          "2": 2.0,
          "3": 3.0,
          "4": 4.0,
          "5": 5.0,
          "6": 6.0,
          "7": 7.0,
          "8": 8.0,
          "9": 9.0
        }),
      },
      AssertSnapshot {
        id: doc_id.clone(),
        index: 0,
        expected: json!({
          "1": 1.0,
          "2": 2.0,
          "3": 3.0,
          "4": 4.0,
          "5": 5.0,
          "6": 6.0,
          "7": 7.0,
          "8": 8.0,
          "9": 9.0,
        }),
      },
    ])
    .await;
}

#[tokio::test]
async fn periodically_gen_snapshot_test() {
  let snapshot_per_update = 5;
  let mut test = CollabPersistenceTest::new(
    CollabPersistenceConfig::new()
      .enable_snapshot(true)
      .snapshot_per_update(snapshot_per_update),
  );
  let doc_id = "1".to_string();
  test
    .run_scripts(vec![OpenDocument { id: doc_id.clone() }])
    .await;

  for i in 0..20 {
    test
      .run_script(InsertKeyValue {
        id: doc_id.clone(),
        key: i.to_string(),
        value: i.into(),
      })
      .await;

    if i != 0 && i % snapshot_per_update == 0 {
      test
        .run_scripts(vec![
          // wait for snapshot to write to disk for 1 second for each snapshot trigger
          Wait(1),
          AssertNumOfUpdates {
            id: doc_id.clone(),
            expected: i as usize + 1,
          },
        ])
        .await;
    }
  }
  // test.run_script(Wait(1)).await;
  test
    .run_script(AssertSnapshot {
      id: doc_id.clone(),
      index: 0,
      expected: json!( {
        "0": 0.0,
        "1": 1.0,
        "2": 2.0,
        "3": 3.0,
        "4": 4.0,
        "5": 5.0
      }),
    })
    .await;

  test
    .run_script(AssertSnapshot {
      id: doc_id.clone(),
      index: 1,
      expected: json!({
        "0": 0.0,
        "1": 1.0,
        "2": 2.0,
        "3": 3.0,
        "4": 4.0,
        "5": 5.0,
        "6": 6.0,
        "7": 7.0,
        "8": 8.0,
        "9": 9.0,
        "10": 10.0
      }),
    })
    .await;

  test
    .run_script(AssertSnapshot {
      id: doc_id.clone(),
      index: 2,
      expected: json!({
        "0": 0.0,
        "1": 1.0,
        "2": 2.0,
        "3": 3.0,
        "4": 4.0,
        "5": 5.0,
        "6": 6.0,
        "7": 7.0,
        "8": 8.0,
        "9": 9.0,
        "10": 10.0,
        "11": 11.0,
        "12": 12.0,
        "13": 13.0,
        "14": 14.0,
        "15": 15.0
      }),
    })
    .await;
  test
    .run_scripts(vec![
      AssertNumOfSnapshots {
        id: doc_id.clone(),
        expected: 3,
      },
      AssertNumOfUpdates {
        id: doc_id,
        expected: 20,
      },
    ])
    .await;
}

#[tokio::test]
async fn gen_big_snapshot_test() {
  let snapshot_per_update = 100;
  let mut test = CollabPersistenceTest::new(
    CollabPersistenceConfig::new()
      .enable_snapshot(true)
      .snapshot_per_update(snapshot_per_update),
  );
  let doc_id = "1".to_string();
  test
    .run_scripts(vec![OpenDocument { id: doc_id.clone() }])
    .await;

  let mut map_1 = serde_json::map::Map::new();
  let mut map_2 = serde_json::map::Map::new();
  for i in 0..300 {
    let s = generate_random_string(100);
    if i < 100 {
      map_1.insert(i.to_string(), json!(&s));
    }

    if i < 200 {
      map_2.insert(i.to_string(), json!(&s));
    }
    if i != 0 && i % snapshot_per_update == 0 {
      test
        .run_scripts(vec![
          // wait for snapshot to write to disk for 1 second for each snapshot trigger
          Wait(1),
        ])
        .await;
    }

    test
      .run_script(InsertKeyValue {
        id: doc_id.clone(),
        key: i.to_string(),
        value: s.into(),
      })
      .await;
  }

  test
    .run_scripts(vec![
      Wait(2),
      AssertSnapshot {
        id: doc_id.clone(),
        index: 0,
        expected: serde_json::Value::Object(map_1),
      },
      AssertSnapshot {
        id: doc_id.clone(),
        index: 1,
        expected: serde_json::Value::Object(map_2),
      },
    ])
    .await;
}

fn generate_random_string(length: usize) -> String {
  const CHARSET: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
  let mut rng = rand::thread_rng();
  let random_string: String = (0..length)
    .map(|_| {
      let index = rng.gen_range(0..CHARSET.len());
      CHARSET[index] as char
    })
    .collect();

  random_string
}
