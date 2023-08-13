use assert_json_diff::assert_json_eq;
use collab_user::core::Reminder;
use serde_json::json;

use crate::util::UserAwarenessTest;

#[tokio::test]
async fn add_reminder_test() {
  let test = UserAwarenessTest::new(1);
  test.lock().add_reminder(Reminder::new(
    "1".to_string(),
    123,
    0,
    "reminder object id".to_string(),
  ));
  let json = test.lock().to_json().unwrap();
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
async fn update_reminder_test() {
  let test = UserAwarenessTest::new(1);
  test.lock().add_reminder(Reminder::new(
    "1".to_string(),
    123,
    0,
    "reminder object id".to_string(),
  ));

  test.lock().update_reminder("1", |reminder| {
    reminder.title = "new title".to_string();
    reminder.message = "new message".to_string();
  });
  let json = test.lock().to_json().unwrap();
  assert_json_eq!(
    json,
    json!({
      "appearance_settings": {},
      "reminders": [
        {
          "id": "1",
          "is_ack": false,
          "message": "new message",
          "reminder_object_id": "reminder object id",
          "scheduled_at": 123,
          "title": "new title",
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
    test.lock().add_reminder(Reminder::new(
      i.to_string(),
      123,
      0,
      format!("reminder object id {}", i),
    ));
  }
  test.lock().remove_reminder("1");
  let json = test.lock().to_json().unwrap();
  assert_json_eq!(
    json,
    json!( {
      "appearance_settings": {},
      "reminders": [
        {
          "id": "0",
          "is_ack": false,
          "message": "",
          "reminder_object_id": "reminder object id 0",
          "scheduled_at": 123,
          "title": "",
          "ty": 0
        },
        {
          "id": "2",
          "is_ack": false,
          "message": "",
          "reminder_object_id": "reminder object id 2",
          "scheduled_at": 123,
          "title": "",
          "ty": 0
        }
      ]
    })
  )
}
