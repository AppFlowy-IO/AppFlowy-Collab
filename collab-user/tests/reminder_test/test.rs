use assert_json_diff::assert_json_eq;
use collab_user::core::Reminder;
use serde_json::json;

use crate::util::UserAwarenessTest;

#[tokio::test]
async fn add_reminder_test() {
  let test = UserAwarenessTest::new(1);
  test.add_reminder(Reminder::new(
    "1".to_string(),
    123,
    0,
    "reminder object id".to_string(),
  ));
  let json = test.to_json().unwrap();
  assert_json_eq!(
    json,
    json!( {
      "appearance_settings": {},
      "reminders": [
        {
          "id": "1",
          "is_ack": false,
          "message": "",
          "reminder_object_id": "reminder object id",
          "scheduled_at": 123,
          "title": "",
          "ty": 0
        }
      ]
    })
  )
}



#[tokio::test]
async fn delete_reminder_test() {
  let test = UserAwarenessTest::new(1);
  for i in 0..3 {
    test.add_reminder(Reminder::new(
      i.to_string(),
      123,
      0,
     format!("reminder object id {}", i)
    ));
  }
  test.remove_reminder("1");
  let json = test.to_json().unwrap();
  assert_json_eq!(
    json,
    json!("")
  )
}
