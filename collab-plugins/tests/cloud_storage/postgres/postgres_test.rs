use crate::cloud_storage::postgres::script::{PostgresStorageTest, TestScript::*};
use collab_plugins::cloud_storage::postgres::SupabaseDBConfig;
use dotenv::dotenv;
use nanoid::nanoid;
use serde_json::json;

/// ⚠️run this test, it will alter the remote table
#[tokio::test]
async fn create_doc_test() {
  dotenv().ok();
  if let Some(config) = SupabaseDBConfig::from_env() {
    let mut test = PostgresStorageTest::new();
    let object_id = nanoid!(10);
    test
      .run_scripts(vec![
        CreateCollab {
          uid: 1,
          object_id: object_id.clone(),
          sync_per_secs: 1,
          config: config.clone(),
        },
        ModifyCollab {
          uid: 1,
          object_id: object_id.clone(),
          f: Box::new(|collab| {
            collab.insert("123", "abc");
          }),
        },
        Wait { secs: 3 },
        AssertLocal {
          uid: 1,
          object_id: object_id.clone(),
          expected: json!( {
            "123": "abc"
          }),
        },
        AssertRemote {
          object_id: object_id.clone(),
          expected: json!( {
            "123": "abc"
          }),
          config,
        },
        Wait { secs: 2 },
      ])
      .await;
  }
}

/// ⚠️run this test, it will alter the remote table
#[tokio::test]
async fn create_multi_docs_test() {
  dotenv().ok();
  if let Some(config) = SupabaseDBConfig::from_env() {
    let mut test = PostgresStorageTest::new();
    let object_id_1 = nanoid!(10);
    let object_id_2 = nanoid!(10);
    test
      .run_scripts(vec![
        CreateCollab {
          uid: 1,
          object_id: object_id_1.clone(),
          sync_per_secs: 1,
          config: config.clone(),
        },
        CreateCollab {
          uid: 1,
          object_id: object_id_2.clone(),
          sync_per_secs: 1,
          config: config.clone(),
        },
        ModifyCollab {
          uid: 1,
          object_id: object_id_1.clone(),
          f: Box::new(|collab| {
            collab.insert("name", "I am object 1");
          }),
        },
        ModifyCollab {
          uid: 1,
          object_id: object_id_2.clone(),
          f: Box::new(|collab| {
            collab.insert("name", "I am object 2");
          }),
        },
        Wait { secs: 3 },
        AssertLocal {
          uid: 1,
          object_id: object_id_1.clone(),
          expected: json!( {
            "name": "I am object 1"
          }),
        },
        AssertRemote {
          object_id: object_id_1.clone(),
          expected: json!( {
            "name": "I am object 1"
          }),
          config: config.clone(),
        },
        AssertLocal {
          uid: 1,
          object_id: object_id_2.clone(),
          expected: json!( {
            "name": "I am object 2"
          }),
        },
        AssertRemote {
          object_id: object_id_2.clone(),
          expected: json!( {
            "name": "I am object 2"
          }),
          config,
        },
        Wait { secs: 2 },
      ])
      .await;
  }
}

// #[tokio::test]
// async fn create_doc_test2() {
//   dotenv().ok();
//   if let Some(config) = SupabaseDBConfig::from_env() {
//     let mut test = PostgresStorageTest::new();
//     test
//       .run_scripts(vec![
//         CreateCollab {
//           uid: 1,
//           object_id: "124e1fb5-fb48-47fd-8fc7-436ae3ec6255".to_string(),
//           sync_per_secs: 1,
//           config: config.clone(),
//         },
//         // AssertRemote {
//         //   object_id: "124e1fb5-fb48-47fd-8fc7-436ae3ec6255".to_string(),
//         //   expected: json!(""),
//         //   config,
//         // },
//         Wait { secs: 2 },
//         AssertLocal {
//           uid: 1,
//           object_id: "124e1fb5-fb48-47fd-8fc7-436ae3ec6255".to_string(),
//           expected: json!(""),
//         },
//         Wait { secs: 2 },
//       ])
//       .await;
//   }
// }
