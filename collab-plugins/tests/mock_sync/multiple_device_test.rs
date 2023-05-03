use crate::util::ScriptTest;
use crate::util::TestScript::*;
use serde_json::json;

#[tokio::test]
async fn write_test() {
  let mut test = ScriptTest::new(1, "1").await;
  test
    .run_scripts(vec![
      CreateClient {
        uid: 1,
        device_id: "1".to_string(),
      },
      CreateEmptyClient {
        uid: 1,
        device_id: "2".to_string(),
      },
      Wait { secs: 1 },
      AssertClientContent {
        device_id: "2".to_string(),
        expected: json!({
          "map": {
            "task1": "a",
            "task2": "b"
          }
        }),
      },
      AssertClientEqualToServer {
        device_id: "1".to_string(),
      },
    ])
    .await;
}
