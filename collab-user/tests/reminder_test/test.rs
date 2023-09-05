use collab_define::reminder::{ObjectType, Reminder};

use crate::util::UserAwarenessTest;
use assert_json_diff::assert_json_eq;
use serde_json::json;

#[tokio::test]
async fn add_reminder_test() {
  let test = UserAwarenessTest::new(1).await;
  let reminder = Reminder::new("1".to_string(), "o1".to_string(), 123, ObjectType::Document)
    .with_key_value("block_id", "fake_block_id")
    .with_key_value("id", "fake_id");
  test.lock().add_reminder(reminder);

  let json = test.lock().to_json().unwrap();
  assert_json_eq!(
    json,
    json!({
      "appearance_settings": {},
      "reminders": [
        {
          "id": "1",
          "object_id": "o1",
          "is_ack": false,
          "message": "",
          "meta": {
            "block_id": "fake_block_id",
            "id": "fake_id"
          },
          "scheduled_at": 123,
          "title": "",
          "ty": 1
        }
      ]
    })
  )
}

#[tokio::test]
async fn update_reminder_test() {
  let test = UserAwarenessTest::new(1).await;
  let reminder = Reminder::new("1".to_string(), "o1".to_string(), 123, ObjectType::Document)
    .with_key_value("block_id", "fake_block_id")
    .with_key_value("id", "fake_id");
  test.lock().add_reminder(reminder);

  test.lock().update_reminder("1", |reminder| {
    reminder.title = "new title".to_string();
    reminder.message = "new message".to_string();
    reminder
      .meta
      .insert("block_id".to_string(), "fake_block_id2".to_string());
  });
  let json = test.lock().to_json().unwrap();
  assert_json_eq!(
    json,
    json!({
      "appearance_settings": {},
      "reminders": [
        {
          "id": "1",
          "object_id": "o1",
          "is_ack": false,
          "message": "new message",
          "meta": {
            "block_id": "fake_block_id2",
            "id": "fake_id"
          },
          "scheduled_at": 123,
          "title": "new title",
          "ty": 1
        }
      ]
    })
  )
}

#[tokio::test]
async fn delete_reminder_test() {
  let test = UserAwarenessTest::new(1).await;
  for i in 0..3 {
    test.lock().add_reminder(Reminder::new(
      i.to_string(),
      "o1".to_string(),
      123,
      ObjectType::Document,
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
          "object_id": "o1",
          "is_ack": false,
          "message": "",
          "meta": {},
          "scheduled_at": 123,
          "title": "",
          "ty": 1
        },
        {
          "id": "2",
          "object_id": "o1",
          "is_ack": false,
          "message": "",
          "meta": {},
          "scheduled_at": 123,
          "title": "",
          "ty": 1
        }
      ]
    })
  )
}
