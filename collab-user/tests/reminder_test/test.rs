use std::collections::HashMap;

use collab_entity::reminder::{ObjectType, Reminder};

use crate::util::UserAwarenessTest;
use assert_json_diff::assert_json_eq;
use serde_json::json;

#[test]
fn add_reminder_test() {
  let mut test = UserAwarenessTest::new(1);
  let reminder = Reminder::new("1".to_string(), "o1".to_string(), 123, ObjectType::Document)
    .with_key_value("block_id", "fake_block_id")
    .with_key_value("id", "fake_id");
  test.add_reminder(reminder);

  let json = test.to_json().unwrap();
  assert_json_eq!(
    json,
    json!({
      "appearance_settings": {},
      "reminders": [
        {
          "id": "1",
          "object_id": "o1",
          "is_ack": false,
          "is_read": false,
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

#[test]
fn update_reminder_test() {
  let mut test = UserAwarenessTest::new(1);
  let reminder = Reminder::new("1".to_string(), "o1".to_string(), 123, ObjectType::Document)
    .with_key_value("block_id", "fake_block_id")
    .with_key_value("id", "fake_id");
  test.add_reminder(reminder);

  test.update_reminder("1", |update| {
    update
      .set_title("new title")
      .set_message("new message")
      .set_meta(HashMap::from([
        ("block_id".to_string(), "fake_block_id2".to_string()),
        ("id".to_string(), "fake_id".to_string()),
      ]));
  });
  let json = test.to_json().unwrap();
  assert_json_eq!(
    json,
    json!({
      "appearance_settings": {},
      "reminders": [
        {
          "id": "1",
          "object_id": "o1",
          "is_ack": false,
          "is_read": false,
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

#[test]
fn update_reminder_multiple_times_test() {
  let mut test = UserAwarenessTest::new(1);
  let reminder = Reminder::new("1".to_string(), "o1".to_string(), 123, ObjectType::Document)
    .with_key_value("block_id", "fake_block_id")
    .with_key_value("id", "fake_id");
  test.add_reminder(reminder);

  test.update_reminder("1", |update| {
    update
      .set_title("new title")
      .set_message("new message")
      .set_meta(HashMap::from([(
        "block_id".to_string(),
        "fake_block_id2".to_string(),
      )]));
  });
  test.update_reminder("1", |update| {
    update.set_title("another title");
  });
  let json = test.to_json().unwrap();
  assert_json_eq!(
    json,
    json!({
      "appearance_settings": {},
      "reminders": [
        {
          "id": "1",
          "object_id": "o1",
          "is_ack": false,
          "is_read": false,
          "message": "new message",
          "meta": {
            "block_id": "fake_block_id2",
          },
          "scheduled_at": 123,
          "title": "another title",
          "ty": 1
        }
      ]
    })
  )
}

#[test]
fn delete_reminder_test() {
  let mut test = UserAwarenessTest::new(1);
  for i in 0..3 {
    test.add_reminder(Reminder::new(
      i.to_string(),
      "o1".to_string(),
      123,
      ObjectType::Document,
    ));
  }
  test.remove_reminder("1");
  let json = test.to_json().unwrap();
  assert_json_eq!(
    json,
    json!( {
      "appearance_settings": {},
      "reminders": [
        {
          "id": "0",
          "object_id": "o1",
          "is_ack": false,
          "is_read": false,
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
          "is_read": false,
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
