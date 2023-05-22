use crate::cloud_storage::postgres::script::{PostgresStorageTest, TestScript::*};
use collab_plugins::cloud_storage::postgres::SupabaseDBConfig;
use dotenv::dotenv;
use nanoid::nanoid;
use serde_json::json;

#[tokio::test]
async fn create_doc_test() {
  dotenv().ok();
  if let Ok(config) = SupabaseDBConfig::from_env() {
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
